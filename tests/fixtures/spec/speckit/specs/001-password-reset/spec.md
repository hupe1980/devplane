# Feature Specification: Password reset

**Feature Branch**: `001-password-reset`

**Created**: 2026-09-20

**Status**: Draft

**Input**: User description: "people who forgot their password can reset it"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Request a reset link (Priority: P1)

A person who forgot their password asks for a link by mail.

**Why this priority**: without it nobody gets back in.

**Independent Test**: request a link and receive one mail.

**Acceptance Scenarios**:

1. **Given** a known address, **When** a reset is requested, **Then** one mail is sent.

---

### User Story 2 - Use the link once (Priority: P2)

The link works exactly once.

**Acceptance Scenarios**:

1. **Given** a used link, **When** it is opened again, **Then** it is refused.

---

### User Story 3 - Expired links (Priority: P3)

A link older than an hour is refused.

### Edge Cases

- What happens when the address is unknown?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST send one reset mail per request.
- **FR-002**: The link MUST expire after an hour.
- **FR-003**: A used link MUST NOT work twice.

## Assumptions

- The mail provider is the one already configured.
