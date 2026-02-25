# TODOs

- [x] Frontend: Fix logout after login, when you log in it puts the token in the URL and it stays there, when you log out and refresh it stays logged in. This is only an issue when the details are in the URL
- [x] Frontend: Add cache busting for assets, and include version in UI somewhere subtle
- [x] Frontend: Populate last-used vehicle by default for fuel entries
- [x] Frontend: Add date and time picker for fuel transactions so they can be back-dated
- [x] Vehicle sharing with other users (with read/write permission levels)
- [x] Security: Should the frontend be using query params? JWTs were intended
- [ ] Backend: Add better integration tests using proper tokens and checking permissions
- [ ] Add bulk transaction import from file
