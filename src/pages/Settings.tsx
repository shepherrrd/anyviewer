import { useState } from "react";
import { Settings as SettingsIcon, Monitor, Network, Shield, Palette } from "lucide-react";
import { useTheme } from "@hooks/useTheme";

type LaunchMode = "startup" | "minimized" | "disabled";

export default function Settings() {
  const { theme, setTheme } = useTheme();
  const [activeTab, setActiveTab] = useState("general");
  const [launchMode, setLaunchMode] = useState<LaunchMode>("startup");
  const [, ] = useState(false);
  const [minimizeToTray, setMinimizeToTray] = useState(true);
  const [alwaysOnTop, setAlwaysOnTop] = useState(true);

  const tabs = [
    { id: "general", name: "General", icon: SettingsIcon },
    { id: "display", name: "Display", icon: Monitor },
    { id: "network", name: "Network", icon: Network },
    { id: "security", name: "Security", icon: Shield },
    { id: "appearance", name: "Appearance", icon: Palette },
  ];

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <div className="flex items-center">
          <div className="flex-shrink-0">
            <div className="w-12 h-12 bg-gray-100 dark:bg-gray-700 rounded-lg flex items-center justify-center">
              <SettingsIcon className="w-6 h-6 text-gray-600 dark:text-gray-400" />
            </div>
          </div>
          <div className="ml-4">
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
              Settings
            </h1>
            <p className="text-gray-600 dark:text-gray-400">
              Configure your AnyViewer preferences
            </p>
          </div>
        </div>
      </div>

      <div className="flex flex-col lg:flex-row gap-6">
        {/* Sidebar */}
        <div className="lg:w-64">
          <nav className="bg-white dark:bg-gray-800 rounded-lg shadow">
            <div className="space-y-1 p-2">
              {tabs.map((tab) => {
                const Icon = tab.icon;
                return (
                  <button
                    key={tab.id}
                    onClick={() => setActiveTab(tab.id)}
                    className={`w-full flex items-center px-3 py-2 text-sm font-medium rounded-md transition-colors ${
                      activeTab === tab.id
                        ? "bg-primary-100 text-primary-900 dark:bg-primary-900 dark:text-primary-100"
                        : "text-gray-600 hover:bg-gray-50 hover:text-gray-900 dark:text-gray-300 dark:hover:bg-gray-700 dark:hover:text-white"
                    }`}
                  >
                    <Icon className="mr-3 h-5 w-5" />
                    {tab.name}
                  </button>
                );
              })}
            </div>
          </nav>
        </div>

        {/* Content */}
        <div className="flex-1">
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
            {activeTab === "general" && (
              <div className="p-6">
                <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-6">
                  General Settings
                </h2>
                <div className="space-y-6">
                  {/* Launch Mode */}
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                      Application Launch Mode
                    </label>
                    <div className="space-y-3">
                      <label className="flex items-center">
                        <input
                          type="radio"
                          name="launchMode"
                          value="startup"
                          checked={launchMode === "startup"}
                          onChange={(e) => setLaunchMode(e.target.value as LaunchMode)}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300"
                        />
                        <div className="ml-3">
                          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            Auto-start with system
                          </span>
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            Launch AnyViewer automatically when system starts
                          </p>
                        </div>
                      </label>
                      <label className="flex items-center">
                        <input
                          type="radio"
                          name="launchMode"
                          value="minimized"
                          checked={launchMode === "minimized"}
                          onChange={(e) => setLaunchMode(e.target.value as LaunchMode)}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300"
                        />
                        <div className="ml-3">
                          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            Start minimized
                          </span>
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            Start with system but minimize to tray
                          </p>
                        </div>
                      </label>
                      <label className="flex items-center">
                        <input
                          type="radio"
                          name="launchMode"
                          value="disabled"
                          checked={launchMode === "disabled"}
                          onChange={(e) => setLaunchMode(e.target.value as LaunchMode)}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300"
                        />
                        <div className="ml-3">
                          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            Manual start only
                          </span>
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            Do not start automatically with system
                          </p>
                        </div>
                      </label>
                    </div>
                  </div>

                  {/* Window Behavior */}
                  <div className="border-t border-gray-200 dark:border-gray-600 pt-6">
                    <h3 className="text-md font-medium text-gray-900 dark:text-white mb-4">
                      Window Behavior
                    </h3>
                    <div className="space-y-4">
                      <div className="flex items-center justify-between">
                        <div>
                          <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            Always on top
                          </label>
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            Keep AnyViewer window above other windows (like AnyDesk)
                          </p>
                        </div>
                        <input
                          type="checkbox"
                          checked={alwaysOnTop}
                          onChange={(e) => setAlwaysOnTop(e.target.checked)}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                        />
                      </div>
                      <div className="flex items-center justify-between">
                        <div>
                          <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            Minimize to tray
                          </label>
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            Minimize to system tray instead of taskbar
                          </p>
                        </div>
                        <input
                          type="checkbox"
                          checked={minimizeToTray}
                          onChange={(e) => setMinimizeToTray(e.target.checked)}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                        />
                      </div>
                    </div>
                  </div>

                  {/* Auto Session */}
                  <div className="border-t border-gray-200 dark:border-gray-600 pt-6">
                    <h3 className="text-md font-medium text-gray-900 dark:text-white mb-4">
                      Session Management
                    </h3>
                    <div className="space-y-4">
                      <div className="flex items-center justify-between">
                        <div>
                          <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            Auto-generate session ID
                          </label>
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            Automatically generate and display session ID on startup
                          </p>
                        </div>
                        <input
                          type="checkbox"
                          defaultChecked
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {activeTab === "display" && (
              <div className="p-6">
                <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-6">
                  Display Settings
                </h2>
                <div className="space-y-6">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Default monitor
                    </label>
                    <select className="input w-full max-w-xs">
                      <option>Primary monitor</option>
                      <option>Monitor 1</option>
                      <option>Monitor 2</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Frame rate (FPS)
                    </label>
                    <input
                      type="range"
                      min="10"
                      max="60"
                      defaultValue="30"
                      className="w-full max-w-xs"
                    />
                    <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                      30 FPS (Higher values use more CPU)
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Quality
                    </label>
                    <input
                      type="range"
                      min="10"
                      max="100" 
                      defaultValue="80"
                      className="w-full max-w-xs"
                    />
                    <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                      80% (Higher values use more bandwidth)
                    </div>
                  </div>
                  <div className="flex items-center justify-between">
                    <div>
                      <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                        Show cursor
                      </label>
                      <p className="text-xs text-gray-500 dark:text-gray-400">
                        Include cursor in screen capture
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
            )}

            {activeTab === "network" && (
              <div className="p-6">
                <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-6">
                  Network Settings
                </h2>
                <div className="space-y-6">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Server port
                    </label>
                    <input
                      type="number"
                      defaultValue="7878"
                      className="input w-full max-w-xs"
                    />
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                      Port for incoming connections
                    </p>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Max connections
                    </label>
                    <input
                      type="number"
                      defaultValue="10"
                      min="1"
                      max="100"
                      className="input w-full max-w-xs"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Connection timeout (seconds)
                    </label>
                    <input
                      type="number"
                      defaultValue="30"
                      min="5"
                      max="300"
                      className="input w-full max-w-xs"
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <div>
                      <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                        Enable discovery
                      </label>
                      <p className="text-xs text-gray-500 dark:text-gray-400">
                        Allow discovery by other AnyViewer instances on the network
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
            )}

            {activeTab === "security" && (
              <div className="p-6">
                <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-6">
                  Security Settings
                </h2>
                <div className="space-y-6">
                  <div className="flex items-center justify-between">
                    <div>
                      <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                        Require authentication
                      </label>
                      <p className="text-xs text-gray-500 dark:text-gray-400">
                        Require password for remote connections
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
                        Enable encryption
                      </label>
                      <p className="text-xs text-gray-500 dark:text-gray-400">
                        Encrypt all network communication
                      </p>
                    </div>
                    <input
                      type="checkbox"
                      className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300 rounded"
                      defaultChecked
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Session timeout (minutes)
                    </label>
                    <input
                      type="number"
                      defaultValue="60"
                      min="5"
                      max="480"
                      className="input w-full max-w-xs"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Max failed attempts
                    </label>
                    <input
                      type="number"
                      defaultValue="5"
                      min="1"
                      max="10"
                      className="input w-full max-w-xs"
                    />
                  </div>
                </div>
              </div>
            )}

            {activeTab === "appearance" && (
              <div className="p-6">
                <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-6">
                  Appearance Settings
                </h2>
                <div className="space-y-6">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                      Theme
                    </label>
                    <div className="space-y-2">
                      <label className="flex items-center">
                        <input
                          type="radio"
                          name="theme"
                          value="light"
                          checked={theme === "light"}
                          onChange={(e) => setTheme(e.target.value as "light")}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300"
                        />
                        <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">
                          Light
                        </span>
                      </label>
                      <label className="flex items-center">
                        <input
                          type="radio"
                          name="theme"
                          value="dark"
                          checked={theme === "dark"}
                          onChange={(e) => setTheme(e.target.value as "dark")}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300"
                        />
                        <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">
                          Dark
                        </span>
                      </label>
                      <label className="flex items-center">
                        <input
                          type="radio"
                          name="theme"
                          value="system"
                          checked={theme === "system"}
                          onChange={(e) => setTheme(e.target.value as "system")}
                          className="h-4 w-4 text-primary-600 focus:ring-primary-500 border-gray-300"
                        />
                        <span className="ml-2 text-sm text-gray-700 dark:text-gray-300">
                          System
                        </span>
                      </label>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {/* Save Button */}
            <div className="border-t border-gray-200 dark:border-gray-700 px-6 py-4">
              <div className="flex justify-end">
                <button className="btn-primary">
                  Save Changes
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}