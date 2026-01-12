use encoding_rs::Encoding;
use std::io::Read;

pub fn read_stdin_text() -> Result<String, std::io::Error> {
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf)?;
    Ok(decode_stdin_bytes(&buf))
}

pub fn decode_stdin_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Ok(enc_name) = std::env::var("MEMEX_STDIN_ENCODING") {
        if let Some(enc) = Encoding::for_label(enc_name.as_bytes()) {
            tracing::debug!(
                "Using MEMEX_STDIN_ENCODING: {}, bytes: {}",
                enc_name,
                bytes.len()
            );
            let (cow, _, _) = enc.decode(bytes);
            return cow.into_owned();
        }
    }

    if let Some((enc, bom_len)) = Encoding::for_bom(bytes) {
        tracing::debug!(
            "Detected BOM encoding: {}, bytes: {}",
            enc.name(),
            bytes.len()
        );
        let (cow, _, _) = enc.decode(&bytes[bom_len..]);
        return cow.into_owned();
    }

    if let Some(enc) = detect_utf16_encoding(bytes) {
        tracing::debug!(
            "Detected UTF-16 encoding: {}, bytes: {}",
            enc.name(),
            bytes.len()
        );
        let (cow, _, _) = enc.decode(bytes);
        return cow.into_owned();
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        tracing::debug!("Valid UTF-8 encoding, bytes: {}", bytes.len());
        return s.to_string();
    }

    #[cfg(windows)]
    {
        for enc in [encoding_rs::GB18030, encoding_rs::GBK] {
            let (cow, _, had_err) = enc.decode(bytes);
            if !had_err {
                tracing::debug!(
                    "Using Windows fallback encoding: {}, bytes: {}",
                    enc.name(),
                    bytes.len()
                );
                return cow.into_owned();
            }
        }
    }

    tracing::debug!("Using UTF-8 lossy conversion, bytes: {}", bytes.len());
    String::from_utf8_lossy(bytes).into_owned()
}

fn detect_utf16_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    let sample_len = bytes.len().min(64);
    if sample_len < 2 {
        return None;
    }

    let mut zero_even = 0;
    let mut zero_odd = 0;
    for (i, b) in bytes.iter().take(sample_len).enumerate() {
        if *b == 0 {
            if i % 2 == 0 {
                zero_even += 1;
            } else {
                zero_odd += 1;
            }
        }
    }

    let threshold = sample_len / 4;
    if zero_odd > threshold && zero_odd > zero_even * 2 {
        return Some(encoding_rs::UTF_16LE);
    }
    if zero_even > threshold && zero_even > zero_odd * 2 {
        return Some(encoding_rs::UTF_16BE);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_bytes() {
        let result = decode_stdin_bytes(&[]);
        assert_eq!(result, "");
    }

    #[test]
    fn test_decode_utf8() {
        let text = "Hello, 世界! 🌍";
        let bytes = text.as_bytes();
        let result = decode_stdin_bytes(bytes);
        assert_eq!(result, text);
    }

    #[test]
    fn test_decode_with_bom() {
        // UTF-8 BOM + text
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("Hello".as_bytes());
        let result = decode_stdin_bytes(&bytes);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_env_override_gbk() {
        // 设置环境变量
        std::env::set_var("MEMEX_STDIN_ENCODING", "gbk");

        // GBK 编码的 "测试" (0xB2, 0xE2, 0xCA, 0xD4)
        let gbk_bytes = vec![0xB2, 0xE2, 0xCA, 0xD4];
        let result = decode_stdin_bytes(&gbk_bytes);

        // 清理环境变量
        std::env::remove_var("MEMEX_STDIN_ENCODING");

        assert_eq!(result, "测试");
    }

    #[test]
    fn test_env_override_utf16le() {
        std::env::set_var("MEMEX_STDIN_ENCODING", "utf-16le");

        // UTF-16LE 编码的 "AB" (0x41, 0x00, 0x42, 0x00)
        let utf16_bytes = vec![0x41, 0x00, 0x42, 0x00];
        let result = decode_stdin_bytes(&utf16_bytes);

        std::env::remove_var("MEMEX_STDIN_ENCODING");

        assert_eq!(result, "AB");
    }

    #[test]
    fn test_env_override_invalid_encoding() {
        std::env::set_var("MEMEX_STDIN_ENCODING", "invalid-encoding-name");

        // 应该回退到正常的 UTF-8 检测
        let text = "Valid UTF-8";
        let result = decode_stdin_bytes(text.as_bytes());

        std::env::remove_var("MEMEX_STDIN_ENCODING");

        assert_eq!(result, text);
    }

    #[test]
    fn test_utf16le_detection() {
        // UTF-16LE 字节序列（多个 null 字节在奇数位）
        let utf16_bytes = vec![
            0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00, // "Hello"
        ];
        let result = decode_stdin_bytes(&utf16_bytes);
        assert_eq!(result, "Hello");
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_gbk_fallback() {
        // GBK 编码的 "中文" (0xD6, 0xD0, 0xCE, 0xC4)
        let gbk_bytes = vec![0xD6, 0xD0, 0xCE, 0xC4];
        let result = decode_stdin_bytes(&gbk_bytes);
        assert_eq!(result, "中文");
    }
}
