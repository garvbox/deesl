use askama::Template;
use axum::{
    Router,
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
};
use diesel::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

use crate::AppState;
use crate::auth::AuthUserRedirect;
use crate::db::DbConn;
use crate::error::AppError;
use crate::models::{StatsAggregator, User, Vehicle};
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
) -> Result<impl IntoResponse, AppError> {
    let user_id = user.user_id;

    let period = params
        .get("period")
        .cloned()
        .unwrap_or_else(|| "all".to_string());
    let cutoff_date = match period.as_str() {
        "7d" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(7)),
        "30d" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(30)),
        "90d" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(90)),
        "1y" => Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(365)),
        _ => None,
    };

    let entries: Vec<(crate::models::FuelEntry, Vehicle)> = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(vehicles::owner_id.eq(user_id))
                .into_boxed();

            let mut query = crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(vehicles::owner_id.eq(user_id))
                .into_boxed();

            if let Some(cutoff) = cutoff_date {
                query = query.filter(crate::schema::fuel_entries::filled_at.gt(cutoff));
            }

            query
                .order(crate::schema::fuel_entries::filled_at.asc())
                .select((crate::models::FuelEntry::as_select(), Vehicle::as_select()))
                .load::<(crate::models::FuelEntry, Vehicle)>(conn)
        })
        .await??;

    let db_user: User = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)
        })
        .await??;

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

        return Ok(Html(template.render()?));
    }

    let mut vehicle_data: HashMap<i32, Vec<(crate::models::FuelEntry, Vehicle)>> = HashMap::new();
    for (entry, vehicle) in entries {
        vehicle_data
            .entry(vehicle.id)
            .or_default()
            .push((entry, vehicle));
    }

    let aggregator = StatsAggregator::calculate(&vehicle_data);

    let vehicle_colors = [
        "rgb(59, 130, 246)",
        "rgb(16, 185, 129)",
        "rgb(139, 92, 246)",
        "rgb(245, 158, 11)",
        "rgb(239, 68, 68)",
        "rgb(14, 165, 233)",
    ];

    let mut all_dates: Vec<String> = Vec::new();
    let mut vehicle_chart_data: HashMap<i32, Vec<(String, f64, f64, f64)>> = HashMap::new();

    for vs in &aggregator.vehicle_stats {
        let entries: Vec<&crate::models::FuelEntry> = vs.entries.iter().collect();
        let mut prev_mileage: Option<i32> = None;
        let mut vehicle_points: Vec<(String, f64, f64, f64)> = Vec::new();

        for entry in &entries {
            if let Some(prev) = prev_mileage {
                let km = entry.km_since(prev);
                if km > 0.0 && entry.litres > 0.0 {
                    let efficiency = entry.efficiency_km_per_litre(prev);
                    let cost_per_km = entry.cost_per_km(prev);
                    let cost_per_litre = entry.cost_per_litre();

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

        vehicle_chart_data.insert(vs.vehicle.id, vehicle_points);
    }

    all_dates.sort();
    let labels: Vec<String> = all_dates
        .into_iter()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut efficiency_series: Vec<ChartSeries> = Vec::new();
    let mut cost_per_km_series: Vec<ChartSeries> = Vec::new();
    let mut cost_per_litre_series: Vec<ChartSeries> = Vec::new();

    for (idx, vs) in aggregator.vehicle_stats.iter().enumerate() {
        let color = vehicle_colors
            .get(idx % vehicle_colors.len())
            .unwrap_or(&"rgb(107, 114, 128)");
        let vehicle_points = vehicle_chart_data
            .get(&vs.vehicle.id)
            .cloned()
            .unwrap_or_default();

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

        let label_text = vs.vehicle.display_name();

        efficiency_series.push(ChartSeries {
            vehicle_id: vs.vehicle.id,
            label: label_text.clone(),
            color: color.to_string(),
            data: eff_data,
        });

        cost_per_km_series.push(ChartSeries {
            vehicle_id: vs.vehicle.id,
            label: label_text.clone(),
            color: color.to_string(),
            data: cpk_data,
        });

        cost_per_litre_series.push(ChartSeries {
            vehicle_id: vs.vehicle.id,
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

    let vehicle_stats: Vec<VehicleStat> = aggregator
        .vehicle_stats
        .into_iter()
        .map(|vs| VehicleStat {
            id: vs.vehicle.id,
            registration: vs.vehicle.registration,
            make: vs.vehicle.make,
            model: vs.vehicle.model,
            total_entries: vs.total_entries,
            total_km: vs.total_km,
            total_litres: vs.total_litres,
            avg_efficiency: vs.avg_efficiency,
            total_cost: vs.total_cost,
        })
        .collect();

    let template = StatsTemplate {
        logged_in: true,
        has_data: true,
        period: period.clone(),
        total_entries: aggregator.total_entries,
        total_litres: aggregator.total_litres,
        total_cost: aggregator.total_cost,
        avg_efficiency: aggregator.avg_efficiency,
        avg_cost_per_km: aggregator.avg_cost_per_km,
        vehicle_stats,
        efficiency_chart,
        cost_per_km_chart,
        cost_per_litre_chart,
        distance_unit: db_user.distance_unit,
        volume_unit: db_user.volume_unit,
    };

    Ok(Html(template.render()?))
}
