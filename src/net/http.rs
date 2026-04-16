use crate::error::Error;

/// An HTTP/1.1 request.
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl HttpRequest {
    /// Serialize the request to HTTP/1.1 wire bytes.
    /// Automatically inserts a `Content-Length` header when a body is present.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();

        // Request line
        out.push_str(&self.method);
        out.push(' ');
        out.push_str(&self.path);
        out.push_str(" HTTP/1.1\r\n");

        // User-supplied headers
        for (name, value) in &self.headers {
            out.push_str(name);
            out.push_str(": ");
            out.push_str(value);
            out.push_str("\r\n");
        }

        // Auto Content-Length when a body is present
        if let Some(body) = &self.body {
            out.push_str("Content-Length: ");
            out.push_str(&body.len().to_string());
            out.push_str("\r\n");
            out.push_str("\r\n");
            out.push_str(body);
        } else {
            out.push_str("\r\n");
        }

        out.into_bytes()
    }
}

/// A parsed HTTP/1.1 response.
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Parse a raw HTTP/1.1 response.
    ///
    /// Supports both `Content-Length` and chunked transfer encoding.
    pub fn parse(raw: &[u8]) -> crate::Result<Self> {
        // Split header section from body at the mandatory blank line (\r\n\r\n).
        let separator = b"\r\n\r\n";
        let sep_pos = raw
            .windows(4)
            .position(|w| w == separator)
            .ok_or_else(|| Error::Http("missing header/body separator".into()))?;

        let header_bytes = &raw[..sep_pos];
        let body_start = &raw[sep_pos + 4..];

        // Parse headers as UTF-8 text
        let header_text = std::str::from_utf8(header_bytes)
            .map_err(|e| Error::Http(format!("invalid UTF-8 in headers: {e}")))?;

        let mut lines = header_text.split("\r\n");

        // Status line: HTTP/1.1 <status> <reason>
        let status_line = lines
            .next()
            .ok_or_else(|| Error::Http("empty response".into()))?;

        let status = parse_status(status_line)?;

        // Header fields
        let mut headers = Vec::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            let colon = line
                .find(':')
                .ok_or_else(|| Error::Http(format!("malformed header: {line}")))?;
            let name = line[..colon].trim().to_string();
            let value = line[colon + 1..].trim().to_string();
            headers.push((name, value));
        }

        // Determine body
        let body = if is_chunked(&headers) {
            decode_chunked(body_start)?
        } else {
            body_start.to_vec()
        };

        Ok(HttpResponse { status, headers, body })
    }

    /// Case-insensitive lookup of a response header value.
    pub fn header(&self, name: &str) -> Option<&str> {
        let name_lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .map(|(_, v)| v.as_str())
    }
}

// ---- helpers ----------------------------------------------------------------

fn parse_status(line: &str) -> crate::Result<u16> {
    // "HTTP/1.1 200 OK"  or  "HTTP/1.0 404 Not Found"
    let mut parts = line.splitn(3, ' ');
    let _version = parts.next();
    let code_str = parts
        .next()
        .ok_or_else(|| Error::Http(format!("malformed status line: {line}")))?;
    code_str
        .parse::<u16>()
        .map_err(|_| Error::Http(format!("invalid status code: {code_str}")))
}

fn is_chunked(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(k, v)| {
        k.to_lowercase() == "transfer-encoding" && v.to_lowercase().contains("chunked")
    })
}

fn decode_chunked(mut src: &[u8]) -> crate::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        // Find the end of the chunk-size line
        let crlf = src
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| Error::Http("chunked: missing CRLF after size".into()))?;

        let size_str = std::str::from_utf8(&src[..crlf])
            .map_err(|_| Error::Http("chunked: non-UTF8 size line".into()))?;

        // Strip optional chunk extensions (";ext=val")
        let size_hex = size_str.split(';').next().unwrap_or("").trim();
        let chunk_len =
            usize::from_str_radix(size_hex, 16).map_err(|_| {
                Error::Http(format!("chunked: invalid size '{size_hex}'"))
            })?;

        src = &src[crlf + 2..]; // advance past size line

        if chunk_len == 0 {
            break; // terminal chunk
        }

        if src.len() < chunk_len + 2 {
            return Err(Error::Http("chunked: truncated chunk data".into()));
        }

        out.extend_from_slice(&src[..chunk_len]);
        src = &src[chunk_len + 2..]; // advance past data + trailing CRLF
    }
    Ok(out)
}
