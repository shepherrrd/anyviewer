import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { Monitor, Wifi, Settings, Activity, Copy, RefreshCw, CheckCircle, Radar, Laptop, Users, Bell, Eye, Clock, User, MapPin } from "lucide-react";
import { Link } from "react-router-dom";
import ConnectionRequestModal from "../components/ConnectionRequestModal";

interface DiscoveredDevice {
  info: {
    device_id: string;
    device_name: string;
    device_type: string;
    version: string;
    capabilities: string[];
    server_port: number;
    ip_address: string;
  };
  last_seen: string;
  address: string;
}

interface ConnectionRequest {
  request_id: string;
  requester_device_id: string;
  requester_name: string;
  requester_ip: string;
  requested_permissions: string[];
  message?: string;
  timestamp: number;
}

export default function Dashboard() {
  const [systemInfo, setSystemInfo] = useState<any>(null);
  const [sessionId, setSessionId] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [generatingId, setGeneratingId] = useState(false);
  const [copied, setCopied] = useState(false);
  const [discoveredDevices, setDiscoveredDevices] = useState<DiscoveredDevice[]>([]);
  const [discoveryActive, setDiscoveryActive] = useState(false);
  const [discoveryLoading, setDiscoveryLoading] = useState(false);
  const [pendingRequest, setPendingRequest] = useState<ConnectionRequest | null>(null);
  const [incomingRequests, setIncomingRequests] = useState<ConnectionRequest[]>([]);
  const [showIncomingRequests, setShowIncomingRequests] = useState(false);
  const [acceptedRequests, setAcceptedRequests] = useState<any[]>([]);
  const [sharingActive, setSharingActive] = useState(false);

  useEffect(() => {
    const loadSystemInfo = async () => {
      try {
        const info = await invoke("get_system_info");
        setSystemInfo(info);
        
        // Auto-start discovery after system info is loaded
        const deviceName = (info as any)?.hostname || "Unknown Device";
        console.log("Auto-starting discovery for device:", deviceName);
        try {
          console.log("Initializing connection request system...");
          await invoke("initialize_connection_requests"); // Initialize connection request system FIRST
          console.log("Connection request system initialized");
          
          console.log("Starting network discovery...");
          await invoke("start_network_discovery", { deviceName });
          setDiscoveryActive(true);
          startDevicePolling();
          
          // Start polling for connection requests and accepted requests
          console.log("Starting polling for connection requests...");
          setInterval(checkForConnectionRequests, 2000);
          setInterval(checkForAcceptedRequests, 3000);
          
          console.log("Auto-discovery started successfully");
        } catch (error) {
          console.error("Failed to auto-start discovery:", error);
        }
      } catch (error) {
        console.error("Failed to load system info:", error);
      } finally {
        setLoading(false);
      }
    };

    const generateSessionId = async () => {
      try {
        setGeneratingId(true);
        // Check if we already have a persistent session ID
        const existingId = localStorage.getItem("anyviewer_session_id");
        if (existingId) {
          setSessionId(existingId);
        } else {
          // Generate new ID and persist it
          const id = await invoke("generate_session_id") as string;
          localStorage.setItem("anyviewer_session_id", id);
          setSessionId(id);
        }
      } catch (error) {
        console.error("Failed to generate session ID:", error);
      } finally {
        setGeneratingId(false);
      }
    };

    loadSystemInfo();
    generateSessionId();
  }, []);

  const copySessionId = async () => {
    try {
      await navigator.clipboard.writeText(sessionId);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      console.error("Failed to copy session ID:", error);
    }
  };

  const regenerateSessionId = async () => {
    try {
      setGeneratingId(true);
      const id = await invoke("generate_session_id") as string;
      localStorage.setItem("anyviewer_session_id", id);
      setSessionId(id);
    } catch (error) {
      console.error("Failed to regenerate session ID:", error);
    } finally {
      setGeneratingId(false);
    }
  };

  const startDiscovery = async () => {
    try {
      setDiscoveryLoading(true);
      const deviceName = systemInfo?.hostname || "Unknown Device";
      console.log("Starting discovery for device:", deviceName);
      await invoke("start_network_discovery", { deviceName });
      setDiscoveryActive(true);
      
      // Start polling for devices
      startDevicePolling();
      console.log("Discovery started successfully");
    } catch (error) {
      console.error("Failed to start discovery:", error);
    } finally {
      setDiscoveryLoading(false);
    }
  };

  const stopDiscovery = async () => {
    try {
      setDiscoveryLoading(true);
      await invoke("stop_network_discovery");
      setDiscoveryActive(false);
      setDiscoveredDevices([]);
    } catch (error) {
      console.error("Failed to stop discovery:", error);
    } finally {
      setDiscoveryLoading(false);
    }
  };

  const refreshDevices = async () => {
    if (!discoveryActive) return;
    
    try {
      const devices = await invoke("get_discovered_devices") as DiscoveredDevice[];
      console.log("Refreshed devices:", devices);
      setDiscoveredDevices(devices);
    } catch (error) {
      console.error("Failed to get discovered devices:", error);
    }
  };

  const startDevicePolling = () => {
    const interval = setInterval(refreshDevices, 2000);
    return () => clearInterval(interval);
  };

  const connectToDevice = async (deviceId: string) => {
    try {
      const device = discoveredDevices.find(d => d.info.device_id === deviceId);
      if (!device) return;

      // Send connection request to the target device via UDP
      const requestId = await invoke("send_connection_request_to_device", {
        deviceId: deviceId,
        requesterName: systemInfo?.hostname || "Unknown Device", 
        requesterIp: "192.168.1.100", // This should be the actual local IP
        requestedPermissions: ["screen_capture", "input_forwarding"],
        message: `Connection request from ${systemInfo?.hostname || "Unknown Device"}`,
      }) as string;

      console.log("Connection request sent to device:", device.info.device_name, "Request ID:", requestId);
      // TODO: Show connection pending state and wait for response
    } catch (error) {
      console.error("Failed to connect to device:", error);
    }
  };

  const checkForConnectionRequests = async () => {
    try {
      const requests = await invoke("get_pending_connection_requests") as ConnectionRequest[];
      console.log("Checking for connection requests, found:", requests.length);
      setIncomingRequests(requests);
      
      // Show the first pending request in modal if none is currently showing
      if (requests.length > 0 && !pendingRequest) {
        console.log("Setting pending request:", requests[0]);
        setPendingRequest(requests[0]);
      }
      
      // If we have requests, show the incoming requests section
      if (requests.length > 0) {
        setShowIncomingRequests(true);
      }
    } catch (error) {
      console.error("Failed to check for connection requests:", error);
    }
  };

  const handleConnectionResponse = (accepted: boolean) => {
    setPendingRequest(null);
    
    // Remove the handled request from incoming requests
    setIncomingRequests(prev => prev.filter(req => req.request_id !== pendingRequest?.request_id));
    
    if (accepted) {
      console.log("Connection request accepted");
      // TODO: Navigate to host session
    } else {
      console.log("Connection request denied");
    }
    
    // Hide incoming requests section if no more requests
    setTimeout(() => {
      checkForConnectionRequests();
    }, 500);
  };

  const viewConnectionRequest = (request: ConnectionRequest) => {
    setPendingRequest(request);
  };

  const checkForAcceptedRequests = async () => {
    try {
      const accepted = await invoke("get_accepted_connection_requests") as any[];
      setAcceptedRequests(accepted);
      
      // If we have accepted requests and screen sharing isn't active, start it
      if (accepted.length > 0 && !sharingActive) {
        for (const request of accepted) {
          await startScreenSharingForRequest(request.request_id);
        }
      }
    } catch (error) {
      console.error("Failed to check for accepted requests:", error);
    }
  };

  const startScreenSharingForRequest = async (requestId: string) => {
    try {
      await invoke("start_screen_sharing_for_request", { requestId });
      setSharingActive(true);
      console.log("Screen sharing started for request:", requestId);
    } catch (error) {
      console.error("Failed to start screen sharing:", error);
    }
  };

  const testIncomingRequest = async () => {
    try {
      console.log("Creating test connection request...");
      const requestId = await invoke("test_create_connection_request") as string;
      console.log("Test request created:", requestId);
      
      // Force check for requests to pick up the new test request
      setTimeout(() => {
        checkForConnectionRequests();
      }, 500);
    } catch (error) {
      console.error("Failed to create test request:", error);
    }
  };

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    if (discoveryActive) {
      cleanup = startDevicePolling();
    }
    return cleanup;
  }, [discoveryActive]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Welcome Section */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">
          Welcome to AnyViewer
        </h1>
        <p className="text-gray-600 dark:text-gray-400">
          Modern remote desktop solution built with Rust and Tauri
        </p>
      </div>

      {/* Session ID Display */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-medium text-gray-900 dark:text-white">
            Your Session ID
          </h2>
          <button
            onClick={regenerateSessionId}
            disabled={generatingId}
            className="btn-secondary text-sm flex items-center"
          >
            <RefreshCw className={`w-4 h-4 mr-2 ${generatingId ? 'animate-spin' : ''}`} />
            {generatingId ? 'Generating...' : 'Regenerate'}
          </button>
        </div>
        
        <div className="bg-gray-50 dark:bg-gray-700 rounded-lg p-4 border-2 border-dashed border-gray-300 dark:border-gray-600">
          <div className="text-center">
            <div className="text-3xl font-mono font-bold text-gray-900 dark:text-white tracking-wider mb-2">
              {sessionId || (generatingId ? '-------' : 'Loading...')}
            </div>
            <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
              Share this ID with others to allow remote access to your computer
            </p>
            
            <div className="flex justify-center space-x-3">
              <button
                onClick={copySessionId}
                disabled={!sessionId || generatingId}
                className="btn-primary text-sm flex items-center"
              >
                {copied ? (
                  <>
                    <CheckCircle className="w-4 h-4 mr-2" />
                    Copied!
                  </>
                ) : (
                  <>
                    <Copy className="w-4 h-4 mr-2" />
                    Copy ID
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
        
        <div className="mt-4 text-xs text-gray-500 dark:text-gray-400 text-center">
          This ID is automatically generated when AnyViewer starts and allows others to connect to your computer.
        </div>
      </div>

      {/* Network Discovery */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-medium text-gray-900 dark:text-white flex items-center">
              <Radar className="w-5 h-5 mr-2" />
              Network Discovery
            </h2>
            <div className="flex space-x-2">
              <button
                onClick={testIncomingRequest}
                className="btn-sm btn-secondary flex items-center"
              >
                <Bell className="w-4 h-4 mr-2" />
                Test Request
              </button>
              <button
                onClick={discoveryActive ? stopDiscovery : startDiscovery}
                disabled={discoveryLoading}
                className={`btn-sm ${discoveryActive ? 'btn-secondary' : 'btn-primary'} flex items-center`}
              >
                {discoveryLoading ? (
                  <>
                    <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                    {discoveryActive ? 'Stopping...' : 'Starting...'}
                  </>
                ) : (
                  <>
                    <Radar className={`w-4 h-4 mr-2 ${discoveryActive ? 'animate-pulse' : ''}`} />
                    {discoveryActive ? 'Stop Discovery' : 'Start Discovery'}
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
        
        <div className="p-6">
          {!discoveryActive ? (
            <div className="text-center py-8">
              <Users className="w-12 h-12 text-gray-400 mx-auto mb-4" />
              <p className="text-gray-500 dark:text-gray-400 mb-4">
                Start network discovery to find devices on your local network
              </p>
              <p className="text-sm text-gray-400 dark:text-gray-500">
                Devices running AnyViewer will automatically appear here
              </p>
            </div>
          ) : discoveredDevices.length === 0 ? (
            <div className="text-center py-8">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto mb-4"></div>
              <p className="text-gray-500 dark:text-gray-400">
                Searching for devices on your network...
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {discoveredDevices.map((device) => (
                <div
                  key={device.info.device_id}
                  className="flex items-center justify-between p-4 border border-gray-200 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
                >
                  <div className="flex items-center">
                    <div className="w-10 h-10 bg-primary-100 dark:bg-primary-900 rounded-lg flex items-center justify-center mr-3">
                      <Laptop className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                    </div>
                    <div>
                      <h3 className="font-medium text-gray-900 dark:text-white">
                        {device.info.device_name}
                      </h3>
                      <p className="text-sm text-gray-500 dark:text-gray-400">
                        {device.info.ip_address} • {device.info.device_type} v{device.info.version}
                      </p>
                      <div className="flex items-center space-x-2 mt-1">
                        {device.info.capabilities.map((capability) => (
                          <span
                            key={capability}
                            className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200"
                          >
                            {capability.replace('_', ' ')}
                          </span>
                        ))}
                      </div>
                    </div>
                  </div>
                  <button
                    onClick={() => connectToDevice(device.info.device_id)}
                    className="btn-primary btn-sm"
                  >
                    Connect
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Active Screen Sharing Status */}
      {sharingActive && (
        <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-4">
          <div className="flex items-center">
            <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse mr-3"></div>
            <div>
              <h3 className="text-sm font-medium text-green-800 dark:text-green-200">
                Screen Sharing Active
              </h3>
              <p className="text-sm text-green-600 dark:text-green-400">
                Your screen is currently being shared with {acceptedRequests.length} connection(s)
              </p>
            </div>
            <button
              onClick={() => {
                setSharingActive(false);
                // TODO: Stop screen sharing
              }}
              className="ml-auto btn-secondary btn-sm"
            >
              Stop Sharing
            </button>
          </div>
        </div>
      )}

      {/* Incoming Connection Requests */}
      {showIncomingRequests && incomingRequests.length > 0 && (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
          <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-medium text-gray-900 dark:text-white flex items-center">
                <Bell className="w-5 h-5 mr-2 text-orange-500" />
                Incoming Connection Requests
                <span className="ml-2 inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200">
                  {incomingRequests.length}
                </span>
              </h2>
              <button
                onClick={() => setShowIncomingRequests(false)}
                className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                ×
              </button>
            </div>
          </div>
          
          <div className="p-6">
            <div className="space-y-3">
              {incomingRequests.map((request) => (
                <div
                  key={request.request_id}
                  className="flex items-center justify-between p-4 border-2 border-orange-200 dark:border-orange-800 rounded-lg bg-orange-50 dark:bg-orange-900/20 hover:bg-orange-100 dark:hover:bg-orange-900/30 transition-colors"
                >
                  <div className="flex items-center">
                    <div className="w-10 h-10 bg-orange-100 dark:bg-orange-900 rounded-lg flex items-center justify-center mr-3">
                      <User className="w-5 h-5 text-orange-600 dark:text-orange-400" />
                    </div>
                    <div>
                      <h3 className="font-medium text-gray-900 dark:text-white">
                        {request.requester_name}
                      </h3>
                      <div className="flex items-center space-x-4 text-sm text-gray-500 dark:text-gray-400">
                        <span className="flex items-center">
                          <MapPin className="w-3 h-3 mr-1" />
                          {request.requester_ip}
                        </span>
                        <span className="flex items-center">
                          <Clock className="w-3 h-3 mr-1" />
                          {new Date(request.timestamp).toLocaleTimeString()}
                        </span>
                      </div>
                      <div className="flex items-center space-x-2 mt-1">
                        {request.requested_permissions.map((permission) => (
                          <span
                            key={permission}
                            className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-orange-100 text-orange-800 dark:bg-orange-800 dark:text-orange-200"
                          >
                            {permission.replace('_', ' ')}
                          </span>
                        ))}
                      </div>
                      {request.message && (
                        <p className="text-sm text-gray-600 dark:text-gray-400 mt-1 italic">
                          "{request.message}"
                        </p>
                      )}
                    </div>
                  </div>
                  <button
                    onClick={() => viewConnectionRequest(request)}
                    className="btn-primary btn-sm flex items-center"
                  >
                    <Eye className="w-4 h-4 mr-2" />
                    View Request
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Quick Actions */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Link
          to="/host"
          className="group bg-white dark:bg-gray-800 rounded-lg shadow p-6 hover:shadow-lg transition-shadow"
        >
          <div className="flex items-center">
            <div className="flex-shrink-0">
              <div className="w-12 h-12 bg-primary-100 dark:bg-primary-900 rounded-lg flex items-center justify-center group-hover:bg-primary-200 dark:group-hover:bg-primary-800 transition-colors">
                <Monitor className="w-6 h-6 text-primary-600 dark:text-primary-400" />
              </div>
            </div>
            <div className="ml-4">
              <h3 className="text-lg font-medium text-gray-900 dark:text-white">
                Start Hosting
              </h3>
              <p className="text-gray-500 dark:text-gray-400">
                Share your screen with others
              </p>
            </div>
          </div>
        </Link>

        <Link
          to="/client"
          className="group bg-white dark:bg-gray-800 rounded-lg shadow p-6 hover:shadow-lg transition-shadow"
        >
          <div className="flex items-center">
            <div className="flex-shrink-0">
              <div className="w-12 h-12 bg-success-100 dark:bg-success-900 rounded-lg flex items-center justify-center group-hover:bg-success-200 dark:group-hover:bg-success-800 transition-colors">
                <Wifi className="w-6 h-6 text-success-600 dark:text-success-400" />
              </div>
            </div>
            <div className="ml-4">
              <h3 className="text-lg font-medium text-gray-900 dark:text-white">
                Connect to Remote
              </h3>
              <p className="text-gray-500 dark:text-gray-400">
                Access someone else's screen
              </p>
            </div>
          </div>
        </Link>
      </div>

      {/* System Information */}
      {systemInfo && (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
          <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-700">
            <h2 className="text-lg font-medium text-gray-900 dark:text-white flex items-center">
              <Activity className="w-5 h-5 mr-2" />
              System Information
            </h2>
          </div>
          <div className="p-6">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
              <div>
                <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                  Operating System
                </dt>
                <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                  {systemInfo.os} ({systemInfo.arch})
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                  Hostname
                </dt>
                <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                  {systemInfo.hostname || "Unknown"}
                </dd>
              </div>
              <div>
                <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                  CPU Cores
                </dt>
                <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                  {systemInfo.cpu_count}
                </dd>
              </div>
              {systemInfo.memory && (
                <div>
                  <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                    Memory
                  </dt>
                  <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                    {systemInfo.memory.total_gb?.toFixed(1)} GB
                  </dd>
                </div>
              )}
              {systemInfo.screens && (
                <div>
                  <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                    Displays
                  </dt>
                  <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                    {systemInfo.screens.length} display{systemInfo.screens.length !== 1 ? 's' : ''}
                  </dd>
                </div>
              )}
              <div>
                <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">
                  Version
                </dt>
                <dd className="mt-1 text-sm text-gray-900 dark:text-white">
                  {systemInfo.app_version}
                </dd>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Quick Settings */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-medium text-gray-900 dark:text-white">
              Settings
            </h2>
            <p className="text-gray-500 dark:text-gray-400">
              Configure your remote desktop preferences
            </p>
          </div>
          <Link
            to="/settings"
            className="btn-primary"
          >
            <Settings className="w-4 h-4 mr-2" />
            Open Settings
          </Link>
        </div>
      </div>

      {/* Connection Request Modal */}
      {pendingRequest && (
        <ConnectionRequestModal
          request={pendingRequest}
          onClose={() => setPendingRequest(null)}
          onResponse={handleConnectionResponse}
        />
      )}
    </div>
  );
}