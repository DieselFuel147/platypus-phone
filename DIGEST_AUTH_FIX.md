# Digest Authentication Fix for INVITE Requests

## Summary

Fixed the SIP INVITE authentication issue that was causing `401 Unauthorized` errors when making calls. The problem was that the `send_with_auth()` function was returning immediately after receiving a **100 Trying** provisional response, instead of waiting for the **401 Unauthorized** challenge.

## The Real Problem

Looking at the log:
1. Initial INVITE sent (CSeq: 1)
2. Server responds with **100 Trying** (provisional response)
3. `send_with_auth()` returns the 100 Trying as the "first response"
4. Server then sends **401 Unauthorized** with auth challenge
5. The 401 arrives in the main loop, but it's too late - the code just fails

The issue: **`send_with_auth()` didn't handle provisional responses (1xx) correctly**.

## Changes Made

### 1. Fixed `send_with_auth()` to Handle Provisional Responses

**Before**: The function would return immediately after receiving ANY response, including provisional 1xx responses.

**After**: The function now:
- Loops and waits for responses
- **Skips provisional responses** (100 Trying, 180 Ringing, 183 Progress)
- Continues waiting until it receives either:
  - A 401/407 auth challenge
  - A final response (2xx, 4xx, 5xx, 6xx)

```rust
// Keep listening for responses until we get a final response or auth challenge
loop {
    // ... receive response ...
    
    // Check if this is a provisional response (1xx)
    if response_str.contains("SIP/2.0 100") || 
       response_str.contains("SIP/2.0 180") || 
       response_str.contains("SIP/2.0 183") {
        println!("[SIP] Provisional response, waiting for final response...");
        buf = vec![0u8; 4096]; // Reset buffer
        continue; // Keep waiting
    }
    
    // Check if authentication is required
    if response_str.contains("SIP/2.0 401") || response_str.contains("SIP/2.0 407") {
        // Handle auth challenge
        break;
    }
    
    // Any other response - return it
    return Ok(response_str);
}
```

### 2. Also Fixed: After Sending Authenticated Request

The same issue existed after sending the authenticated INVITE - the function would return a provisional response instead of waiting for the final response.

**Solution**: Added another loop after sending the authenticated request to skip provisional responses and wait for the final response.

### 3. Fixed ACK CSeq (from previous fix)

The ACK request now uses `CSeq: 2 ACK` to match the authenticated INVITE's sequence number.

## How It Works Now

### Proper INVITE Flow with Authentication:

1. **Initial INVITE** (CSeq: 1)
   - Sent WITHOUT Authorization header

2. **100 Trying** (provisional)
   - **SKIPPED** - keep waiting

3. **401 Unauthorized** (final response)
   - Parse `WWW-Authenticate` header
   - Extract `realm`, `nonce`, `qop`, `algorithm`

4. **Calculate Digest Response**
   - When `qop=auth` is present:
     ```
     HA1 = MD5(username:realm:password)
     HA2 = MD5(INVITE:sip:destination@server)
     response = MD5(HA1:nonce:nc:cnonce:qop:HA2)
     ```
   - Includes `nc=00000001` and random `cnonce`

5. **Authenticated INVITE** (CSeq: 2)
   - New branch parameter
   - Incremented CSeq
   - Same Call-ID and From tag
   - Includes Authorization header with proper digest

6. **100 Trying** (provisional)
   - **SKIPPED** - keep waiting

7. **180 Ringing** or **200 OK** (final response)
   - Return this response to caller

8. **ACK** (CSeq: 2)
   - ACK CSeq matches the INVITE CSeq
   - Uses the To tag from 200 OK response
   - New branch parameter

## What Was Already Correct

The existing implementation already had:

✅ Proper `qop=auth` handling in `calculate_digest_response()`  
✅ Correct URI in digest (uses exact destination URI from INVITE line)  
✅ CSeq incrementing on auth retry  
✅ Branch parameter changing on retry  
✅ Call-ID and From tag consistency  
✅ Proper `nc` and `cnonce` generation  

## Testing

To test the fix:

1. Register with your SIP server
2. Make a call to another extension
3. Check the logs for:
   - Initial INVITE with CSeq: 1
   - 100 Trying (skipped)
   - 401 response with WWW-Authenticate
   - Authenticated INVITE with CSeq: 2
   - 100 Trying (skipped)
   - 180 Ringing or 200 OK
   - ACK with CSeq: 2

## Root Cause Analysis

The original analysis about Digest authentication was partially correct - the digest calculation was already working properly. The **actual bug** was simpler:

**The `send_with_auth()` function didn't understand SIP's provisional vs final response model.**

In SIP:
- **Provisional responses (1xx)**: Indicate progress, not final outcome
- **Final responses (2xx-6xx)**: Indicate the final result of the request

The function was treating the first response (100 Trying) as final, when it should have kept waiting for the actual final response (401 Unauthorized).

This is a common mistake when implementing SIP clients because:
- HTTP doesn't have provisional responses
- The 100 Trying response looks like a "success" if you're not familiar with SIP
- The auth challenge comes AFTER the provisional response

## Additional Notes

The fix properly handles:
- Multiple provisional responses (100, 180, 183)
- Auth challenges that come after provisional responses
- Provisional responses after authenticated requests
- Proper response filtering and waiting logic

The code now correctly implements RFC 3261's transaction model for INVITE requests with authentication.

## References

- **RFC 3261**: SIP - Session Initiation Protocol (Section 17 - Transactions)
- **RFC 2617**: HTTP Digest Authentication
- **RFC 3264**: Offer/Answer Model with SDP
