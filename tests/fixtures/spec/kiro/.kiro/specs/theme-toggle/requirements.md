# Requirements Document

## Introduction

A theme toggle for the settings page.

## Requirements

### Requirement 1

**User Story:** As a user, I want a dark theme, so that the page is readable at night.

#### Acceptance Criteria

1. WHEN the user presses the toggle THEN the system SHALL switch the theme
2. WHEN the page reloads THEN the system SHALL keep the chosen theme

### Requirement 2

**User Story:** As a user, I want the theme to follow the system, so that I set it once.

#### Acceptance Criteria

1. WHEN the system theme changes THEN the system SHALL follow it
2. IF the user chose a theme THEN the system SHALL NOT follow the system theme

### Requirement 3

**User Story:** As a user, I want a high-contrast theme, so that I can read the page.

#### Acceptance Criteria

1. WHEN high contrast is chosen THEN the system SHALL meet the WCAG AA contrast ratio
