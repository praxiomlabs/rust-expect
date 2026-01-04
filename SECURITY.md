# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in rust-expect, please report it by:

1. **Do NOT** open a public GitHub issue
2. Use GitHub's private vulnerability reporting feature at [Security Advisories](https://github.com/praxiomlabs/rust-expect/security/advisories/new)
3. Include detailed information about the vulnerability
4. Allow reasonable time for a fix before public disclosure (typically 90 days)

## Scope

This security policy covers:

- The rust-expect library
- The rust-expect-macros crate
- The rust-pty crate

## Security Considerations

### PII Redaction

The `pii-redaction` feature provides best-effort redaction of sensitive data in logs. It is **not** a security boundary and should not be relied upon to prevent data leakage in adversarial scenarios.

### SSH Sessions

When using the `ssh` feature:

- Private keys are handled in memory; ensure proper memory protection
- Credentials should not be logged (even with PII redaction enabled)
- Use agent forwarding or key-based authentication when possible

#### Known Vulnerability: RUSTSEC-2023-0071 (Marvin Attack)

**Status:** Unpatched upstream (as of January 2025)

The `ssh` feature depends on the `russh` crate, which transitively depends on the `rsa` crate. The RSA crate has a known timing sidechannel vulnerability ([RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071)) that could potentially allow private key recovery through timing analysis in networked environments.

**Impact:**
- RSA key operations may leak timing information observable over the network
- In theory, this could enable private key recovery by a sophisticated attacker
- Local use on non-compromised systems is generally safe

**Mitigation:**
- **STRONGLY RECOMMENDED:** Use Ed25519 keys instead of RSA keys for SSH authentication
- The default credential helper (`SshCredentials::with_defaults()`) already prioritizes `~/.ssh/id_ed25519` before `~/.ssh/id_rsa`
- If you must use RSA keys, avoid using them in environments where attackers can observe network timing

**Workaround:** Generate and use Ed25519 keys:
```bash
ssh-keygen -t ed25519 -C "your_email@example.com"
```

This issue is tracked upstream in the RustCrypto/RSA repository. We will update our dependencies when a fixed version becomes available.

#### Host Key Verification

By default, rust-expect uses `HostKeyVerification::KnownHosts` which validates SSH server keys against the user's `~/.ssh/known_hosts` file. This prevents man-in-the-middle attacks.

The `HostKeyVerification::AcceptAll` variant (which disables host key verification) is gated behind the `insecure-skip-verify` feature flag. This flag is:
- **NOT** included in the `full` feature bundle
- **NOT** recommended for production use
- **ONLY** intended for controlled testing environments

To enable it (at your own risk):
```toml
[dependencies]
rust-expect = { version = "0.1", features = ["ssh", "insecure-skip-verify"] }
```

### PTY Operations

PTY operations run with the privileges of the calling process. Be cautious when:

- Spawning processes with elevated privileges
- Handling user-provided input
- Logging session content

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Best Practices

1. Keep dependencies updated
2. Use `cargo audit` regularly
3. Review session logs before sharing
4. Use environment variables for sensitive configuration
5. Run tests in isolated environments
