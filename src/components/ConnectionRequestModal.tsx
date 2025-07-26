import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { X, Monitor, Shield, Clock, Wifi } from "lucide-react";

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
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 w-full max-w-md mx-4">
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