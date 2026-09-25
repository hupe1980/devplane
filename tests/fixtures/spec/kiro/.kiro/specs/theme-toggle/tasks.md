# Implementation Plan

- [ ] 1. Set up the theme context
  - Create a ThemeProvider holding the current theme
  - _Requirements: 1.1_

- [ ] 2. Persist and follow the system theme
  - [ ] 2.1 Persist the choice
    - Write the choice to local storage
    - _Requirements: 1.2, 2.2_
  - [x] 2.2 Listen for the media query
    - _Requirements: 2.1_

- [ ] 3. Write the end-to-end tests
  - Cover toggling and reloading
