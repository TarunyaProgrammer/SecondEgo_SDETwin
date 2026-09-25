# Security Policy

## Supported versions

The project is pre-release. Security fixes are applied to the current default branch; older snapshots are not guaranteed to receive fixes.

## Reporting a vulnerability

Do not open a public issue for an undisclosed vulnerability. Report it privately to the repository maintainers through the private security-reporting mechanism available on the hosting platform. Include:

- a clear description and impact;
- affected commit, file, or component;
- reproducible steps or a minimal proof of concept;
- suggested mitigation, if known.

Allow maintainers reasonable time to investigate before public disclosure. Do not include real credentials, personal data, or destructive payloads in a report.

## Security expectations for contributors

- Never commit API keys, tokens, private keys, credentials, or `.env` files.
- Treat model output, tool arguments, repository content, and shell output as untrusted input.
- Keep command execution and file mutation within explicit workspace boundaries.
- Validate paths to prevent traversal and unintended writes.
- Use timeouts and bounded retries for external processes.
- Avoid logging secrets or full sensitive file contents in telemetry.

