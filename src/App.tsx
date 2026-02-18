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
  const [showSettings, setShowSettings] = useState(false);
  const [inputDevices, setInputDevices] = useState<string[]>([]);
  const [outputDevices, setOutputDevices] = useState<string[]>([]);
  const [selectedInput, setSelectedInput] = useState("");
  const [selectedOutput, setSelectedOutput] = useState("");
  const [testResult, setTestResult] = useState("");

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

  const loadAudioDevices = async () => {
    try {
      const inputs = await invoke<string[]>("list_audio_input_devices");
      const outputs = await invoke<string[]>("list_audio_output_devices");
      setInputDevices(inputs);
      setOutputDevices(outputs);
      if (inputs.length > 0 && !selectedInput) setSelectedInput(inputs[0]);
      if (outputs.length > 0 && !selectedOutput) setSelectedOutput(outputs[0]);
    } catch (error) {
      console.error("Failed to load audio devices:", error);
      setTestResult(`Error: ${error}`);
    }
  };

  const handleTestMicrophone = async () => {
    setTestResult("Testing microphone...");
    try {
      const result = await invoke<string>("test_microphone", {
        deviceName: selectedInput || null,
      });
      setTestResult(result);
    } catch (error) {
      setTestResult(`Test failed: ${error}`);
    }
  };

  const openSettings = () => {
    setShowSettings(true);
    loadAudioDevices();
  };

  return (
    <div className="container">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h1>🦆 Platypus Phone</h1>
        <button onClick={openSettings} style={{ fontSize: "1.5em", padding: "0.5em 1em" }}>
          ⚙️ Audio Settings
        </button>
      </div>
      
      <div className={`status ${getStatusClass()}`}>
        <h3>Status: {callState}</h3>
        <p>{isRegistered ? "✓ Registered" : "✗ Not Registered"}</p>
      </div>

      {showSettings && (
        <div className="modal-overlay" onClick={() => setShowSettings(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h2>🎤 Audio Settings</h2>
            
            <div style={{ marginBottom: "1.5em" }}>
              <h3>Microphone</h3>
              <select 
                value={selectedInput} 
                onChange={(e) => setSelectedInput(e.target.value)}
                style={{ width: "100%", padding: "0.5em", marginBottom: "0.5em" }}
              >
                {inputDevices.length === 0 && <option>No devices found</option>}
                {inputDevices.map((device) => (
                  <option key={device} value={device}>{device}</option>
                ))}
              </select>
              <button onClick={handleTestMicrophone} style={{ width: "100%" }}>
                🎤 Test Microphone
              </button>
            </div>

            <div style={{ marginBottom: "1.5em" }}>
              <h3>Speaker</h3>
              <select 
                value={selectedOutput} 
                onChange={(e) => setSelectedOutput(e.target.value)}
                style={{ width: "100%", padding: "0.5em" }}
              >
                {outputDevices.length === 0 && <option>No devices found</option>}
                {outputDevices.map((device) => (
                  <option key={device} value={device}>{device}</option>
                ))}
              </select>
            </div>

            {testResult && (
              <div style={{ 
                padding: "1em", 
                backgroundColor: testResult.includes("Error") || testResult.includes("failed") ? "#ffebee" : "#e8f5e9",
                borderRadius: "4px",
                marginBottom: "1em"
              }}>
                {testResult}
              </div>
            )}

            <div style={{ marginTop: "1em" }}>
              <button onClick={loadAudioDevices} style={{ marginRight: "0.5em" }}>
                🔄 Refresh Devices
              </button>
              <button onClick={() => setShowSettings(false)}>
                Close
              </button>
            </div>

            {inputDevices.length === 0 && outputDevices.length === 0 && (
              <div style={{ 
                marginTop: "1.5em", 
                padding: "1em", 
                backgroundColor: "#fff3cd",
                borderRadius: "4px"
              }}>
                <strong>⚠️ No audio devices found!</strong>
                <p style={{ marginTop: "0.5em", fontSize: "0.9em" }}>
                  You're running in WSL. To use audio, you need to build and run the Windows version.
                  <br/><br/>
                  Run: <code>npm run tauri build</code>
                  <br/>
                  Then find the .exe in: <code>src-tauri/target/release/</code>
                </p>
              </div>
            )}
          </div>
        </div>
      )}

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
