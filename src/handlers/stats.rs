use askama::Template;
use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use diesel::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

use super::internal_error;
use crate::AppState;
use crate::auth::AuthUserRedirect;
use crate::db::DbConn;
use crate::models::{FuelStation, User, Vehicle};
use crate::schema::{users, vehicles};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(stats_page))
}

#[derive(Template)]
#[template(path = "stats.html")]
pub struct StatsTemplate {
    pub logged_in: bool,
    pub has_data: bool,
    pub period: String,
    pub total_entries: usize,
    pub total_litres: f64,
    pub total_cost: f64,
    pub avg_efficiency: f64,
    pub avg_cost_per_km: f64,
    pub vehicle_stats: Vec<VehicleStat>,
    pub efficiency_chart: ChartData,
    pub cost_per_km_chart: ChartData,
    pub cost_per_litre_chart: ChartData,
    pub distance_unit: String,
    pub volume_unit: String,
}

#[derive(Serialize)]
pub struct VehicleStat {
    pub id: i32,
    pub registration: String,
    pub make: String,
    pub model: String,
    pub total_entries: i64,
    pub total_km: f64,
    pub total_litres: f64,
    pub avg_efficiency: f64,
    pub total_cost: f64,
}

#[derive(Serialize, Clone)]
pub struct ChartSeries {
    pub vehicle_id: i32,
    pub label: String,
    pub color: String,
    pub data: Vec<Option<f64>>, // None for dates where this vehicle has no entry
}

#[derive(Serialize, Clone)]
pub struct ChartData {
    pub labels: Vec<String>,
    pub series: Vec<ChartSeries>,
}

pub async fn stats_page(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    // Parse time period (default to "all")
    let period = params
        .get("period")
        .cloned()
        .unwrap_or_else(|| "all".to_string());
    let cutoff_date = match period.as_str() {
        "7d" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(7)),
        "30d" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(30)),
        "90d" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(90)),
        "1y" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(365)),
        _ => None, // "all" or invalid
    };

    // Get all fuel entries for the user's vehicles with vehicle and station info
    let entries: Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)> = conn
        .interact(move |conn| {
            let mut query = crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .left_join(crate::schema::fuel_stations::table)
                .filter(vehicles::owner_id.eq(user_id))
                .into_boxed();

            if let Some(cutoff) = cutoff_date {
                query = query.filter(crate::schema::fuel_entries::filled_at.gt(cutoff));
            }

            query
                .order(crate::schema::fuel_entries::filled_at.asc())
                .select((
                    crate::models::FuelEntry::as_select(),
                    Vehicle::as_select(),
                    Option::<FuelStation>::as_select(),
                ))
                .load::<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let db_user: User = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if entries.is_empty() {
        let empty_chart = ChartData {
            labels: vec![],
            series: vec![],
        };
        let template = StatsTemplate {
            logged_in: true,
            has_data: false,
            period: period.clone(),
            total_entries: 0,
            total_litres: 0.0,
            total_cost: 0.0,
            avg_efficiency: 0.0,
            avg_cost_per_km: 0.0,
            vehicle_stats: vec![],
            efficiency_chart: empty_chart.clone(),
            cost_per_km_chart: empty_chart.clone(),
            cost_per_litre_chart: empty_chart,
            distance_unit: db_user.distance_unit,
            volume_unit: db_user.volume_unit,
        };

        return Ok(Html(template.render().map_err(internal_error)?));
    }

    // Calculate overall stats
    let total_entries = entries.len();
    let total_litres: f64 = entries.iter().map(|(e, _, _)| e.litres).sum();
    let total_cost: f64 = entries.iter().map(|(e, _, _)| e.cost).sum();

    // Group entries by vehicle
    let mut vehicle_data: HashMap<
        i32,
        Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>,
    > = HashMap::new();
    for (entry, vehicle, station) in entries {
        vehicle_data
            .entry(vehicle.id)
            .or_default()
            .push((entry, vehicle.clone(), station));
    }

    // Vehicle colors for charts
    let vehicle_colors = [
        "rgb(59, 130, 246)", // Blue
        "rgb(16, 185, 129)", // Green
        "rgb(139, 92, 246)", // Purple
        "rgb(245, 158, 11)", // Orange
        "rgb(239, 68, 68)",  // Red
        "rgb(14, 165, 233)", // Sky
    ];

    // Calculate vehicle stats and collect all unique dates
    let mut vehicle_stats = Vec::new();
    let mut all_dates: Vec<String> = Vec::new();
    let mut vehicle_chart_data: HashMap<i32, Vec<(String, f64, f64, f64)>> = HashMap::new(); // vehicle_id -> [(date, efficiency, cost_per_km, cost_per_litre)]

    for (vehicle_id, vehicle_entries) in vehicle_data.iter() {
        let vehicle = &vehicle_entries[0].1;
        let mut total_km = 0.0;
        let mut vehicle_litres = 0.0;
        let mut vehicle_cost = 0.0;
        let mut prev_mileage: Option<i32> = None;
        let mut efficiency_sum = 0.0;
        let mut efficiency_count = 0;
        let mut vehicle_points: Vec<(String, f64, f64, f64)> = Vec::new();

        for (entry, _, _) in vehicle_entries.iter() {
            vehicle_litres += entry.litres;
            vehicle_cost += entry.cost;

            if let Some(prev) = prev_mileage {
                let km = (entry.mileage_km - prev) as f64;
                if km > 0.0 && entry.litres > 0.0 {
                    let efficiency = km / entry.litres;
                    let cost_per_km = entry.cost / km;
                    let cost_per_litre = entry.cost / entry.litres;
                    efficiency_sum += efficiency;
                    efficiency_count += 1;
                    total_km += km;

                    let date_label = entry.filled_at.format("%Y-%m-%d").to_string();
                    vehicle_points.push((
                        date_label.clone(),
                        efficiency,
                        cost_per_km,
                        cost_per_litre,
                    ));

                    if !all_dates.contains(&date_label) {
                        all_dates.push(date_label);
                    }
                }
            }
            prev_mileage = Some(entry.mileage_km);
        }

        let avg_efficiency = if efficiency_count > 0 {
            efficiency_sum / efficiency_count as f64
        } else {
            0.0
        };

        vehicle_stats.push(VehicleStat {
            id: *vehicle_id,
            registration: vehicle.registration.clone(),
            make: vehicle.make.clone(),
            model: vehicle.model.clone(),
            total_entries: vehicle_entries.len() as i64,
            total_km,
            total_litres: vehicle_litres,
            avg_efficiency,
            total_cost: vehicle_cost,
        });

        vehicle_chart_data.insert(*vehicle_id, vehicle_points);
    }

    // Sort dates and take last 20
    all_dates.sort();
    let labels: Vec<String> = all_dates
        .into_iter()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    // Build chart series for each vehicle
    let mut efficiency_series: Vec<ChartSeries> = Vec::new();
    let mut cost_per_km_series: Vec<ChartSeries> = Vec::new();
    let mut cost_per_litre_series: Vec<ChartSeries> = Vec::new();

    for (idx, vehicle_stat) in vehicle_stats.iter().enumerate() {
        let color = vehicle_colors
            .get(idx % vehicle_colors.len())
            .unwrap_or(&"rgb(107, 114, 128)");
        let vehicle_points = vehicle_chart_data
            .get(&vehicle_stat.id)
            .cloned()
            .unwrap_or_default();

        // Create a map of date -> (efficiency, cost_per_km, cost_per_litre) for this vehicle
        let mut data_by_date: HashMap<&str, (f64, f64, f64)> = HashMap::new();
        for (date, eff, cpk, cpl) in &vehicle_points {
            data_by_date.insert(date.as_str(), (*eff, *cpk, *cpl));
        }

        let mut eff_data: Vec<Option<f64>> = Vec::new();
        let mut cpk_data: Vec<Option<f64>> = Vec::new();
        let mut cpl_data: Vec<Option<f64>> = Vec::new();

        for label in &labels {
            if let Some((eff, cpk, cpl)) = data_by_date.get(label.as_str()) {
                eff_data.push(Some(*eff));
                cpk_data.push(Some(*cpk));
                cpl_data.push(Some(*cpl));
            } else {
                eff_data.push(None);
                cpk_data.push(None);
                cpl_data.push(None);
            }
        }

        let label_text = format!("{} {}", vehicle_stat.make, vehicle_stat.model);

        efficiency_series.push(ChartSeries {
            vehicle_id: vehicle_stat.id,
            label: label_text.clone(),
            color: color.to_string(),
            data: eff_data,
        });

        cost_per_km_series.push(ChartSeries {
            vehicle_id: vehicle_stat.id,
            label: label_text.clone(),
            color: color.to_string(),
            data: cpk_data,
        });

        cost_per_litre_series.push(ChartSeries {
            vehicle_id: vehicle_stat.id,
            label: label_text,
            color: color.to_string(),
            data: cpl_data,
        });
    }

    let efficiency_chart = ChartData {
        labels: labels.clone(),
        series: efficiency_series,
    };

    let cost_per_km_chart = ChartData {
        labels: labels.clone(),
        series: cost_per_km_series,
    };

    let cost_per_litre_chart = ChartData {
        labels,
        series: cost_per_litre_series,
    };

    // Calculate averages
    let avg_efficiency = if !vehicle_stats.is_empty() {
        vehicle_stats.iter().map(|v| v.avg_efficiency).sum::<f64>() / vehicle_stats.len() as f64
    } else {
        0.0
    };

    let avg_cost_per_km = if !vehicle_stats.is_empty() {
        let total_km: f64 = vehicle_stats.iter().map(|v| v.total_km).sum();
        let total_cost_sum: f64 = vehicle_stats.iter().map(|v| v.total_cost).sum();
        if total_km > 0.0 {
            total_cost_sum / total_km
        } else {
            0.0
        }
    } else {
        0.0
    };

    let template = StatsTemplate {
        logged_in: true,
        has_data: true,
        period: period.clone(),
        total_entries,
        total_litres,
        total_cost,
        avg_efficiency,
        avg_cost_per_km,
        vehicle_stats,
        efficiency_chart,
        cost_per_km_chart,
        cost_per_litre_chart,
        distance_unit: db_user.distance_unit,
        volume_unit: db_user.volume_unit,
    };

    Ok(Html(template.render().map_err(internal_error)?))
}
