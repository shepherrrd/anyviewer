import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { Wifi, MonitorSpeaker, WifiOff } from "lucide-react";

interface ConnectionResponse {
  success: boolean;
  session_id: string;
  server_info?: {
    version: string;
    capabilities: string[];
    encryption_enabled: boolean;
  };
  error?: string;
}

export default function ClientSession() {
  const [connectionInput, setConnectionInput] = useState("");
  const [connectionType, setConnectionType] = useState<"auto" | "session" | "ip">("auto");
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [connectionInfo, setConnectionInfo] = useState<ConnectionResponse | null>(null);
  const [error, setError] = useState<string>("");

  // Auto-detect connection type based on input format
  const detectConnectionType = (input: string): "session" | "ip" => {
    // IP address pattern (basic check for IPv4)
    const ipPattern = /^(\d{1,3}\.){3}\d{1,3}(:\d+)?$/;
    // Session ID pattern (8 characters, numbers/letters)
    const sessionPattern = /^[A-Za-z0-9]{8}$/;
    
    if (ipPattern.test(input)) {
      return "ip";
    } else if (sessionPattern.test(input.replace(/[-\s]/g, ""))) {
      return "session";
    }
    
    // Default to session ID for anything else
    return "session";
  };

  const connectToRemote = async () => {
    if (!connectionInput.trim()) {
      setError("Please enter a session ID or IP address");
      return;
    }

    setIsConnecting(true);
    setError("");

    try {
      const input = connectionInput.trim();
      const actualConnectionType = connectionType === "auto" ? detectConnectionType(input) : connectionType;
      
      let response: ConnectionResponse;
      
      if (actualConnectionType === "ip") {
        // Connect using IP address for local network
        response = await invoke<ConnectionResponse>("connect_to_ip", { 
          ipAddress: input,
          port: input.includes(":") ? undefined : "5900" // Default VNC-like port
        });
      } else {
        // Connect using session ID through relay server
        response = await invoke<ConnectionResponse>("connect_to_session", { 
          sessionId: input.replace(/[-\s]/g, "") // Clean session ID
        });
      }
      
      if (response.success) {
        setConnectionInfo(response);
        setIsConnected(true);
      } else {
        setError(response.error || "Failed to connect");
      }
    } catch (err) {
      setError(err as string);
    } finally {
      setIsConnecting(false);
    }
  };

  const disconnect = async () => {
    setIsConnected(false);
    setConnectionInfo(null);
    setConnectionInput("");
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center">
          <div className="flex-shrink-0">
            <div className="w-12 h-12 bg-success-100 dark:bg-success-900 rounded-lg flex items-center justify-center">
              <Wifi className="w-6 h-6 text-success-600 dark:text-success-400" />
            </div>
          </div>
          <div className="ml-4">
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
              Connect to Remote Desktop
            </h1>
            <p className="text-gray-600 dark:text-gray-400">
              Enter a session ID or IP address to connect to a remote screen
            </p>
          </div>
        </div>
      </div>

      {/* Connection Form */}
      {!isConnected ? (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
          <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
            Connection Details
          </h2>

          {error && (
            <div className="mb-4 p-4 bg-error-50 dark:bg-error-900 border border-error-200 dark:border-error-700 rounded-md">
              <p className="text-error-700 dark:text-error-200">{error}</p>
            </div>
          )}

          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                Connection Type
              </label>
              <select
                value={connectionType}
                onChange={(e) => setConnectionType(e.target.value as "auto" | "session" | "ip")}
                className="input w-full mb-3"
                disabled={isConnecting}
              >
                <option value="auto">Auto-detect</option>
                <option value="session">Session ID (via relay server)</option>
                <option value="ip">IP Address (local network)</option>
              </select>
            </div>
            
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                {connectionType === "ip" ? "IP Address" : connectionType === "session" ? "Session ID" : "Session ID or IP Address"}
              </label>
              <input
                type="text"
                value={connectionInput}
                onChange={(e) => setConnectionInput(e.target.value)}
                placeholder={
                  connectionType === "ip" 
                    ? "192.168.1.100 or 192.168.1.100:5900" 
                    : connectionType === "session"
                    ? "12345678 or 123-456-789"
                    : "192.168.1.100 or 12345678"
                }
                className="input w-full"
                disabled={isConnecting}
              />
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {connectionType === "ip" 
                  ? "Enter the IP address of the computer you want to connect to" 
                  : connectionType === "session"
                  ? "Get this ID from the person sharing their screen"
                  : "Enter either a session ID (for remote connections) or IP address (for local network)"
                }
              </p>
            </div>

            <div className="flex justify-end">
              <button
                onClick={connectToRemote}
                disabled={isConnecting || !connectionInput.trim()}
                className="btn-primary"
              >
                {isConnecting ? (
                  <>
                    <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                    Connecting...
                  </>
                ) : (
                  <>
                    <Wifi className="w-4 h-4 mr-2" />
                    Connect
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      ) : (
        /* Connected State */
        <div className="space-y-6">
          {/* Connection Status */}
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-medium text-gray-900 dark:text-white">
                Connected to Remote Desktop
              </h2>
              <div className="status-connected">
                Connected
              </div>
            </div>

            {connectionInfo && (
              <div className="space-y-3">
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                      Session ID
                    </dt>
                    <dd className="mt-1 text-sm text-gray-900 dark:text-white font-mono">
                      {connectionInfo.session_id}
                    </dd>
                  </div>
                  {connectionInfo.server_info && (
                    <>
                      <div>
                        <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                          Server Version
                        </dt>
                        <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                          {connectionInfo.server_info.version}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                          Encryption
                        </dt>
                        <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                          {connectionInfo.server_info.encryption_enabled ? "Enabled" : "Disabled"}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                          Capabilities
                        </dt>
                        <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                          {connectionInfo.server_info.capabilities.join(", ")}
                        </dd>
                      </div>
                    </>
                  )}
                </div>

                <div className="flex justify-end">
                  <button
                    onClick={disconnect}
                    className="btn-danger"
                  >
                    <WifiOff className="w-4 h-4 mr-2" />
                    Disconnect
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Remote Screen Viewer */}
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-medium text-gray-900 dark:text-white">
                Remote Screen
              </h2>
              <div className="flex items-center space-x-2 text-sm text-gray-500 dark:text-gray-400">
                <MonitorSpeaker className="w-4 h-4" />
                <span>1920x1080</span>
              </div>
            </div>

            {/* Placeholder for remote screen */}
            <div className="bg-gray-100 dark:bg-gray-700 rounded-lg aspect-video flex items-center justify-center">
              <div className="text-center">
                <MonitorSpeaker className="mx-auto h-12 w-12 text-gray-400 mb-4" />
                <p className="text-gray-500 dark:text-gray-400">
                  Remote screen will appear here
                </p>
                <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
                  Waiting for screen data...
                </p>
              </div>
            </div>
          </div>

          {/* Connection Controls */}
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
            <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
              Connection Controls
            </h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                    Enable remote control
                  </label>
                  <p className="text-xs text-gray-500 dark:text-gray-400">
                    Allow mouse and keyboard input
                  </p>
                </div>
                <input
                  type="checkbox"
                  className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                  defaultChecked
                />
              </div>
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                    Fit to window
                  </label>
                  <p className="text-xs text-gray-500 dark:text-gray-400">
                    Scale remote screen to fit
                  </p>
                </div>
                <input
                  type="checkbox"
                  className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                  defaultChecked
                />
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}