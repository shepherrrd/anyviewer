import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { Monitor, Wifi, Settings, Activity, Copy, RefreshCw, CheckCircle } from "lucide-react";
import { Link } from "react-router-dom";

export default function Dashboard() {
  const [systemInfo, setSystemInfo] = useState<any>(null);
  const [sessionId, setSessionId] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [generatingId, setGeneratingId] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const loadSystemInfo = async () => {
      try {
        const info = await invoke("get_system_info");
        setSystemInfo(info);
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
    </div>
  );
}