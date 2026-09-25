# Tasks: Password reset

**Input**: Design documents from `/specs/001-password-reset/`

**Prerequisites**: plan.md, spec.md

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Create the mail template in src/mail/reset.html

## Phase 3: User Story 1 - Request a reset link (Priority: P1) 🎯 MVP

- [x] T002 [US1] Add the reset route in src/routes/reset.rs
- [ ] T003 [P] [US1] Send one mail per request in src/mail/send.rs

## Phase 4: User Story 2 - Use the link once (Priority: P2)

- [ ] T004 [US2] Mark a used link in src/auth/reset.rs
- [ ] T005 [US9] Rate-limit the endpoint in src/routes/reset.rs

```
- [ ] T999 [US3] An example in a fence
```
