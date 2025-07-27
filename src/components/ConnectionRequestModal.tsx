import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { X, Monitor, Shield, Clock, Wifi, Eye, EyeOff } from "lucide-react";

interface ConnectionRequest {
  request_id: string;
  requester_device_id: string;
  requester_name: string;
  requester_ip: string;
  requested_permissions: string[];
  message?: string;
  timestamp: number;
}

interface ConnectionRequestModalProps {
  request: ConnectionRequest;
  onClose: () => void;
  onResponse: (accepted: boolean) => void;
}

export default function ConnectionRequestModal({ request, onClose, onResponse }: ConnectionRequestModalProps) {
  const [responding, setResponding] = useState(false);
  const [screenPreview, setScreenPreview] = useState<string | null>(null);
  const [showPreview, setShowPreview] = useState(false);
  const [loadingPreview, setLoadingPreview] = useState(false);

  const handleAccept = async () => {
    setResponding(true);
    try {
      await invoke("respond_to_connection_request", {
        requestId: request.request_id,
        accepted: true,
        grantedPermissions: request.requested_permissions,
        sessionDurationMinutes: 60, // 1 hour default
        denialReason: null,
      });
      
      // Start screen sharing immediately if screen_capture permission is granted
      if (request.requested_permissions.includes('screen_capture')) {
        try {
          await invoke("start_screen_sharing_for_request", {
            requestId: request.request_id,
          });
          console.log("Screen sharing started for accepted request");
        } catch (error) {
          console.error("Failed to start screen sharing:", error);
        }
      }
      
      onResponse(true);
    } catch (error) {
      console.error("Failed to accept connection request:", error);
    } finally {
      setResponding(false);
    }
  };

  const handleDeny = async () => {
    setResponding(true);
    try {
      await invoke("respond_to_connection_request", {
        requestId: request.request_id,
        accepted: false,
        grantedPermissions: [],
        sessionDurationMinutes: null,
        denialReason: "Connection denied by user",
      });
      onResponse(false);
    } catch (error) {
      console.error("Failed to deny connection request:", error);
    } finally {
      setResponding(false);
    }
  };

  const captureScreenPreview = async () => {
    setLoadingPreview(true);
    try {
      console.log("Capturing screen preview...");
      const screenData = await invoke("capture_screen") as number[];
      console.log("Screen data received, length:", screenData.length);
      
      // Convert the array to Uint8Array
      const uint8Array = new Uint8Array(screenData);
      
      // Create blob from the raw image data
      const blob = new Blob([uint8Array], { type: "image/png" });
      const url = URL.createObjectURL(blob);
      
      console.log("Created blob URL:", url);
      setScreenPreview(url);
    } catch (error) {
      console.error("Failed to capture screen preview:", error);
      // Set a placeholder image or error state
      setScreenPreview(null);
    } finally {
      setLoadingPreview(false);
    }
  };

  const togglePreview = () => {
    if (!showPreview && !screenPreview) {
      captureScreenPreview();
    }
    setShowPreview(!showPreview);
  };

  useEffect(() => {
    return () => {
      // Cleanup blob URL when component unmounts
      if (screenPreview) {
        URL.revokeObjectURL(screenPreview);
      }
    };
  }, [screenPreview]);

  const formatPermissions = (permissions: string[]) => {
    const permissionLabels: { [key: string]: { label: string; icon: any } } = {
      screen_capture: { label: "View your screen", icon: Monitor },
      input_forwarding: { label: "Control mouse and keyboard", icon: Shield },
      file_transfer: { label: "Transfer files", icon: Clock },
    };

    return permissions.map(perm => permissionLabels[perm] || { label: perm.replace('_', ' '), icon: Shield });
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 w-full max-w-lg mx-4 max-h-screen overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center">
            <div className="w-10 h-10 bg-blue-100 dark:bg-blue-900 rounded-lg flex items-center justify-center mr-3">
              <Wifi className="w-5 h-5 text-blue-600 dark:text-blue-400" />
            </div>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
              Connection Request
            </h2>
          </div>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Request Details */}
        <div className="mb-6">
          <div className="bg-gray-50 dark:bg-gray-700 rounded-lg p-4 mb-4">
            <h3 className="font-medium text-gray-900 dark:text-white mb-2">
              {request.requester_name}
            </h3>
            <p className="text-sm text-gray-600 dark:text-gray-400">
              IP Address: {request.requester_ip}
            </p>
            {request.message && (
              <p className="text-sm text-gray-600 dark:text-gray-400 mt-2">
                Message: "{request.message}"
              </p>
            )}
          </div>

          <div className="mb-4">
            <h4 className="text-sm font-medium text-gray-900 dark:text-white mb-2">
              Requested Permissions:
            </h4>
            <div className="space-y-2">
              {formatPermissions(request.requested_permissions).map((perm, index) => {
                const IconComponent = perm.icon;
                return (
                  <div key={index} className="flex items-center text-sm text-gray-600 dark:text-gray-400">
                    <IconComponent className="w-4 h-4 mr-2" />
                    {perm.label}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Screen Preview Section */}
          {request.requested_permissions.includes('screen_capture') && (
            <div className="mb-4">
              <div className="flex items-center justify-between mb-2">
                <h4 className="text-sm font-medium text-gray-900 dark:text-white">
                  Screen Preview:
                </h4>
                <button
                  onClick={togglePreview}
                  disabled={loadingPreview}
                  className="text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 flex items-center"
                >
                  {loadingPreview ? (
                    "Loading..."
                  ) : showPreview ? (
                    <>
                      <EyeOff className="w-4 h-4 mr-1" />
                      Hide Preview
                    </>
                  ) : (
                    <>
                      <Eye className="w-4 h-4 mr-1" />
                      Show Preview
                    </>
                  )}
                </button>
              </div>
              
              {showPreview && (
                <div className="border border-gray-200 dark:border-gray-600 rounded-lg overflow-hidden">
                  {loadingPreview ? (
                    <div className="w-full h-32 bg-gray-100 dark:bg-gray-700 flex items-center justify-center">
                      <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600"></div>
                      <span className="ml-2 text-sm text-gray-500">Capturing...</span>
                    </div>
                  ) : screenPreview ? (
                    <img
                      src={screenPreview}
                      alt="Screen preview"
                      className="w-full h-32 object-cover"
                      onError={() => {
                        console.error("Failed to load screen preview image");
                        setScreenPreview(null);
                      }}
                    />
                  ) : (
                    <div className="w-full h-32 bg-gray-100 dark:bg-gray-700 flex items-center justify-center">
                      <div className="text-center">
                        <Monitor className="w-8 h-8 text-gray-400 mx-auto mb-2" />
                        <p className="text-xs text-gray-500">Preview unavailable</p>
                      </div>
                    </div>
                  )}
                </div>
              )}
              
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                This shows what the remote user will be able to see
              </p>
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="flex space-x-3">
          <button
            onClick={handleDeny}
            disabled={responding}
            className="flex-1 px-4 py-2 text-sm font-medium text-gray-700 bg-gray-200 border border-gray-300 rounded-md hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 disabled:opacity-50"
          >
            {responding ? "Processing..." : "Deny"}
          </button>
          <button
            onClick={handleAccept}
            disabled={responding}
            className="flex-1 px-4 py-2 text-sm font-medium text-white bg-blue-600 border border-transparent rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50"
          >
            {responding ? "Processing..." : "Accept"}
          </button>
        </div>

        {/* Warning */}
        <div className="mt-4 p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg">
          <p className="text-xs text-yellow-800 dark:text-yellow-200">
            ⚠️ Only accept connections from devices you trust. The remote user will have the permissions listed above.
          </p>
        </div>
      </div>
    </div>
  );
}