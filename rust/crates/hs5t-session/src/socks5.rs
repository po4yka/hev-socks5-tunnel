use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Authentication method for SOCKS5 handshake.
#[derive(Debug, Clone)]
pub enum Auth {
    NoAuth,
    UserPass { username: String, password: String },
}

/// Perform SOCKS5 handshake (method negotiation + optional auth).
///
/// On success the stream is ready for a CONNECT/ASSOCIATE request.
pub async fn handshake<S>(stream: &mut S, auth: &Auth) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // Method byte: 0x00 = NO_AUTH, 0x02 = USERNAME_PASSWORD
    let method = match auth {
        Auth::NoAuth => 0x00u8,
        Auth::UserPass { .. } => 0x02u8,
    };

    // Send greeting: VER=5, NMETHODS=1, METHOD
    stream.write_all(&[0x05, 0x01, method]).await?;

    // Read server method selection: [VER, METHOD]
    let mut resp = [0u8; 2];
    stream.read_exact(&mut resp).await?;

    if resp[1] == 0xFF {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SOCKS5: no acceptable authentication method",
        ));
    }

    // Sub-authentication for USERNAME_PASSWORD
    if let Auth::UserPass { username, password } = auth {
        let ulen = username.len() as u8;
        let plen = password.len() as u8;

        let mut req = Vec::with_capacity(3 + username.len() + password.len());
        req.push(0x01); // sub-negotiation version
        req.push(ulen);
        req.extend_from_slice(username.as_bytes());
        req.push(plen);
        req.extend_from_slice(password.as_bytes());
        stream.write_all(&req).await?;

        // Read sub-auth response: [VER, STATUS]
        let mut auth_resp = [0u8; 2];
        stream.read_exact(&mut auth_resp).await?;

        if auth_resp[1] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "SOCKS5: authentication rejected",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::io::Builder;

    // -------------------------------------------------------------------------
    // NoAuth
    // -------------------------------------------------------------------------

    /// Client MUST send [0x05, 0x01, 0x00] and accept [0x05, 0x00] from server.
    #[tokio::test]
    async fn noauth_handshake_sends_correct_greeting() {
        let mut mock = Builder::new()
            // Expect handshake() to write the greeting
            .write(&[0x05, 0x01, 0x00])
            // Server selects NO_AUTH (0x00)
            .read(&[0x05, 0x00])
            .build();

        handshake(&mut mock, &Auth::NoAuth).await.unwrap();
    }

    /// Server responding with 0xFF (no acceptable method) must yield an error.
    #[tokio::test]
    async fn noauth_handshake_rejects_no_acceptable_method() {
        let mut mock = Builder::new()
            .write(&[0x05, 0x01, 0x00])
            // Server rejects all methods
            .read(&[0x05, 0xFF])
            .build();

        let result = handshake(&mut mock, &Auth::NoAuth).await;
        assert!(result.is_err(), "expected error when server returns 0xFF");
    }

    // -------------------------------------------------------------------------
    // UserPass
    // -------------------------------------------------------------------------

    /// Full UserPass flow: greeting → method selection → sub-auth.
    ///
    /// Wire format of sub-authentication request:
    ///   [0x01, len(user), ...user_bytes, len(pass), ...pass_bytes]
    /// Wire format of sub-authentication response:
    ///   [0x01, 0x00]  → success
    #[tokio::test]
    async fn userpass_handshake_sends_correct_bytes() {
        let user = b"alice";
        let pass = b"s3cr3t";

        let greeting_bytes = vec![0x05u8, 0x01, 0x02];
        let mut auth_bytes = vec![0x01u8, user.len() as u8];
        auth_bytes.extend_from_slice(user);
        auth_bytes.push(pass.len() as u8);
        auth_bytes.extend_from_slice(pass);

        let mut mock = Builder::new()
            // Step 1: greeting advertises UserPass (0x02)
            .write(&greeting_bytes)
            // Step 2: server selects UserPass
            .read(&[0x05, 0x02])
            // Step 3: client sends credentials
            .write(&auth_bytes)
            // Step 4: server accepts
            .read(&[0x01, 0x00])
            .build();

        let auth = Auth::UserPass {
            username: "alice".to_string(),
            password: "s3cr3t".to_string(),
        };
        handshake(&mut mock, &auth).await.unwrap();
    }

    /// Server returning a non-zero status byte in sub-auth response is an error.
    #[tokio::test]
    async fn userpass_handshake_fails_on_auth_rejection() {
        let user = b"alice";
        let pass = b"wrong";

        let greeting_bytes = vec![0x05u8, 0x01, 0x02];
        let mut auth_bytes = vec![0x01u8, user.len() as u8];
        auth_bytes.extend_from_slice(user);
        auth_bytes.push(pass.len() as u8);
        auth_bytes.extend_from_slice(pass);

        let mut mock = Builder::new()
            .write(&greeting_bytes)
            .read(&[0x05, 0x02])
            .write(&auth_bytes)
            // Server rejects credentials (non-zero status)
            .read(&[0x01, 0x01])
            .build();

        let auth = Auth::UserPass {
            username: "alice".to_string(),
            password: "wrong".to_string(),
        };
        let result = handshake(&mut mock, &auth).await;
        assert!(result.is_err(), "expected error when server rejects credentials");
    }
}
