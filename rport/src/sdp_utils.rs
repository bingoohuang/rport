/// Strip IPv6 candidates from SDP content
pub fn strip_ipv6_candidates(sdp: &str) -> String {
    sdp.lines()
        .filter(|line| {
            // Filter out IPv6 ICE candidates
            if line.starts_with("a=candidate:") {
                // Check if the line contains IPv6 address
                // IPv6 addresses contain colons and are typically longer
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let ip = parts[4];
                    // Simple check for IPv6: contains colons and multiple segments
                    if ip.contains(':') && ip.matches(':').count() > 1 {
                        return false; // Filter out IPv6 candidates
                    }
                }
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ipv6_candidates() {
        let sdp_with_ipv6 = r#"v=0
o=- 123456789 123456789 IN IP4 192.168.1.1
s=-
t=0 0
a=candidate:1 1 UDP 2130706431 192.168.1.1 54400 typ host
a=candidate:2 1 UDP 2130706431 2001:db8::1 54401 typ host
a=candidate:3 1 UDP 2130706431 10.0.0.1 54402 typ host
m=application 9 UDP/DTLS/SCTP webrtc-datachannel"#;

        let result = strip_ipv6_candidates(sdp_with_ipv6);
        
        // Should contain IPv4 candidates
        assert!(result.contains("192.168.1.1"));
        assert!(result.contains("10.0.0.1"));
        
        // Should not contain IPv6 candidates
        assert!(!result.contains("2001:db8::1"));
    }
}
