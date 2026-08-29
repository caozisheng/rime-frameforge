# Development Rules

## Local DNG fixtures

- Local DNG tests and smoke verification MUST use fixtures from `C:\Users\zisheng\Documents\cao\99_data\isp\pana_gh5s`.
- The `essentials-for-ci` release asset (`P1020601.dng`) is reserved for repository CI setup and MUST NOT be required for local development tests.
- When a local test expects `pipeline/normal/P1020601.dng`, copy or link the appropriate local GH5S fixture into that ignored path temporarily; do not commit the fixture or change CI download configuration.
