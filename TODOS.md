# TODOs

- [x] Axum cors setup is wide open - needs to be tightened. We should only allow open CORS in dev, for production
  we want it to be restrictive as it will be on the same host/port.
- [ ] Need to quickly access fuel from landing page rather than browsing vehicle
- [ ] Render dates properly
- [ ] Use user-defined currency and render symbol in UI. Default is Euro
- [ ] Use OAuth for sign-in instead of manual user and password
- [ ] Users should be added in a disabled state by default until approved by an admin user. Add a simple CLI tool for managing users so that we can bootstrap the first admin

