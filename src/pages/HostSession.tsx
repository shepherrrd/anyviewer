import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { Monitor, Copy, StopCircle, Users, Activity } from "lucide-react";

export default function HostSession() {
  const [isHosting, setIsHosting] = useState(false);
  const [sessionId, setSessionId] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>("");

  const startHosting = async () => {
    setLoading(true);
    setError("");
    
    try {
      const id = await invoke<string>("start_host_session");
      setSessionId(id);
      setIsHosting(true);
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  const stopHosting = async () => {
    setLoading(true);
    
    try {
      // In a real implementation, you'd call a stop_host_session command
      setIsHosting(false);
      setSessionId("");
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  const copySessionId = async () => {
    try {
      await navigator.clipboard.writeText(sessionId);
      // You could show a toast notification here
    } catch (err) {
      console.error("Failed to copy session ID:", err);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center">
          <div className="flex-shrink-0">
            <div className="w-12 h-12 bg-primary-100 dark:bg-primary-900 rounded-lg flex items-center justify-center">
              <Monitor className="w-6 h-6 text-primary-600 dark:text-primary-400" />
            </div>
          </div>
          <div className="ml-4">
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
              Host Session
            </h1>
            <p className="text-gray-600 dark:text-gray-400">
              Share your screen with remote users
            </p>
          </div>
        </div>
      </div>

      {/* Status Card */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-medium text-gray-900 dark:text-white">
            Session Status
          </h2>
          <div className={`status-indicator ${isHosting ? 'status-connected' : 'status-disconnected'}`}>
            {isHosting ? 'Hosting' : 'Not Hosting'}
          </div>
        </div>

        {error && (
          <div className="mb-4 p-4 bg-error-50 dark:bg-error-900 border border-error-200 dark:border-error-700 rounded-md">
            <p className="text-error-700 dark:text-error-200">{error}</p>
          </div>
        )}

        {!isHosting ? (
          <div className="text-center py-8">
            <Monitor className="mx-auto h-12 w-12 text-gray-400 mb-4" />
            <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">
              Start sharing your screen
            </h3>
            <p className="text-gray-500 dark:text-gray-400 mb-6">
              Click the button below to start a host session and get a session ID
            </p>
            <button
              onClick={startHosting}
              disabled={loading}
              className="btn-primary"
            >
              {loading ? (
                <>
                  <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                  Starting...
                </>
              ) : (
                <>
                  <Monitor className="w-4 h-4 mr-2" />
                  Start Hosting
                </>
              )}
            </button>
          </div>
        ) : (
          <div className="space-y-6">
            {/* Session ID */}
            <div className="bg-gray-50 dark:bg-gray-700 rounded-lg p-4">
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                Session ID
              </label>
              <div className="flex items-center space-x-2">
                <div className="flex-1 font-mono text-lg bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded-md px-3 py-2">
                  {sessionId}
                </div>
                <button
                  onClick={copySessionId}
                  className="btn-outline"
                  title="Copy Session ID"
                >
                  <Copy className="w-4 h-4" />
                </button>
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                Share this ID with users who want to connect to your screen
              </p>
            </div>

            {/* Session Controls */}
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-4">
                <div className="flex items-center text-sm text-gray-500 dark:text-gray-400">
                  <Users className="w-4 h-4 mr-1" />
                  0 connected users
                </div>
                <div className="flex items-center text-sm text-gray-500 dark:text-gray-400">
                  <Activity className="w-4 h-4 mr-1" />
                  Active session
                </div>
              </div>
              <button
                onClick={stopHosting}
                disabled={loading}
                className="btn-danger"
              >
                {loading ? (
                  <>
                    <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white mr-2"></div>
                    Stopping...
                  </>
                ) : (
                  <>
                    <StopCircle className="w-4 h-4 mr-2" />
                    Stop Hosting
                  </>
                )}
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Settings */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
          Host Settings
        </h2>
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                Allow remote control
              </label>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Let connected users control your mouse and keyboard
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
                Show cursor
              </label>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Display your cursor in the shared screen
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
  );
}