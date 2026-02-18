import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import { usePhoneStore } from "./store";

function App() {
  const { 
    callState, 
    phoneNumber, 
    isRegistered,
    setCallState,
    setPhoneNumber,
    setIsRegistered 
  } = usePhoneStore();

  const [sipServer, setSipServer] = useState("");
  const [sipUser, setSipUser] = useState("");
  const [sipPassword, setSipPassword] = useState("");

  useEffect(() => {
    // Listen for SIP events from Rust backend
    const unlisten = listen("sip-event", (event: any) => {
      console.log("SIP Event:", event.payload);
      
      if (event.payload.type === "registration_state") {
        setIsRegistered(event.payload.registered);
        setCallState(event.payload.registered ? "REGISTERED" : "INITIALIZED");
      } else if (event.payload.type === "call_state") {
        setCallState(event.payload.state);
      }
    });

    // Initialize SIP stack
    invoke("init_sip").then(() => {
      console.log("SIP stack initialized");
      setCallState("INITIALIZED");
    }).catch(console.error);

    return () => {
      unlisten.then(f => f());
    };
  }, [setCallState, setIsRegistered]);

  const handleRegister = async () => {
    try {
      setCallState("REGISTERING");
      await invoke("register_account", {
        server: sipServer,
        user: sipUser,
        password: sipPassword,
      });
    } catch (error) {
      console.error("Registration failed:", error);
      setCallState("INITIALIZED");
    }
  };

  const handleCall = async () => {
    if (!phoneNumber) return;
    
    try {
      await invoke("make_call", { number: phoneNumber });
      setCallState("OUTGOING");
    } catch (error) {
      console.error("Call failed:", error);
    }
  };

  const handleHangup = async () => {
    try {
      await invoke("hangup_call");
    } catch (error) {
      console.error("Hangup failed:", error);
    }
  };

  const handleAnswer = async () => {
    try {
      await invoke("answer_call");
    } catch (error) {
      console.error("Answer failed:", error);
    }
  };

  const addDigit = (digit: string) => {
    setPhoneNumber(phoneNumber + digit);
  };

  const getStatusClass = () => {
    if (isRegistered) return "connected";
    if (callState === "REGISTERING") return "connecting";
    return "disconnected";
  };

  return (
    <div className="container">
      <h1>🦆 Platypus Phone</h1>
      
      <div className={`status ${getStatusClass()}`}>
        <h3>Status: {callState}</h3>
        <p>{isRegistered ? "✓ Registered" : "✗ Not Registered"}</p>
      </div>

      {!isRegistered && (
        <div style={{ margin: "2em 0" }}>
          <h3>SIP Account</h3>
          <div>
            <input
              type="text"
              placeholder="SIP Server (e.g., sip.example.com)"
              value={sipServer}
              onChange={(e) => setSipServer(e.target.value)}
            />
          </div>
          <div>
            <input
              type="text"
              placeholder="Username"
              value={sipUser}
              onChange={(e) => setSipUser(e.target.value)}
            />
          </div>
          <div>
            <input
              type="password"
              placeholder="Password"
              value={sipPassword}
              onChange={(e) => setSipPassword(e.target.value)}
            />
          </div>
          <button onClick={handleRegister}>Register</button>
        </div>
      )}

      {isRegistered && (
        <>
          <div>
            <input
              type="text"
              className="phone-input"
              placeholder="Enter phone number"
              value={phoneNumber}
              onChange={(e) => setPhoneNumber(e.target.value)}
            />
          </div>

          <div className="dialpad">
            {["1", "2", "3", "4", "5", "6", "7", "8", "9", "*", "0", "#"].map(
              (digit) => (
                <button key={digit} onClick={() => addDigit(digit)}>
                  {digit}
                </button>
              )
            )}
          </div>

          <div className="call-controls">
            {callState === "REGISTERED" && (
              <button onClick={handleCall} disabled={!phoneNumber}>
                📞 Call
              </button>
            )}
            
            {callState === "INCOMING" && (
              <button className="answer" onClick={handleAnswer}>
                ✓ Answer
              </button>
            )}
            
            {(callState === "ACTIVE" || callState === "OUTGOING" || callState === "INCOMING") && (
              <button className="hangup" onClick={handleHangup}>
                ✗ Hangup
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}

export default App;
