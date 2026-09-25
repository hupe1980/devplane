## ADDED Requirements

### Requirement: Rate limiting

The API SHALL refuse the 101st request from one client in a minute.

#### Scenario: Over the limit

- **WHEN** a client sends 101 requests in a minute
- **THEN** the last one is refused

### Requirement: Retry hint

The refusal SHALL say when to retry.

#### Scenario: Refused

- **WHEN** a request is refused
- **THEN** the response carries a `Retry-After` header
