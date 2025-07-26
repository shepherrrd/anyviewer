import { Routes, Route } from "react-router-dom";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import Sidebar from "@components/Layout/Sidebar";
import Header from "@components/Layout/Header";
import Dashboard from "@pages/Dashboard";
import HostSession from "@pages/HostSession";
import ClientSession from "@pages/ClientSession";
import Settings from "@pages/Settings";
import { useTheme } from "@hooks/useTheme";
import { cn } from "@utils/cn";

function App() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [systemInfo, setSystemInfo] = useState<any>(null);
  const { theme } = useTheme();

  useEffect(() => {
    // Initialize the app
    const initializeApp = async () => {
      try {
        // Get system information
        const info = await invoke("get_system_info");
        setSystemInfo(info);

        // Initialize security
        await invoke("initialize_security");

        console.log("AnyViewer initialized successfully");
      } catch (error) {
        console.error("Failed to initialize AnyViewer:", error);
      }
    };

    initializeApp();

    // Listen for system tray events
    const unlistenStartHost = listen("start-host-requested", () => {
      console.log("Start host requested from system tray");
      // Navigate to host session page or trigger host start
    });

    const unlistenStopHost = listen("stop-host-requested", () => {
      console.log("Stop host requested from system tray");
      // Stop hosting or navigate away
    });

    return () => {
      unlistenStartHost.then(fn => fn());
      unlistenStopHost.then(fn => fn());
    };
  }, []);

  return (
    <div className={cn("min-h-screen bg-gray-50 dark:bg-gray-900", theme)}>
      <div className="flex h-screen">
        {/* Sidebar */}
        <Sidebar 
          open={sidebarOpen} 
          onClose={() => setSidebarOpen(false)} 
        />

        {/* Main Content */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Header */}
          <Header 
            onMenuClick={() => setSidebarOpen(true)}
            systemInfo={systemInfo}
          />

          {/* Main Content Area */}
          <main className="flex-1 overflow-y-auto p-6">
            <Routes>
              <Route path="/" element={<Dashboard />} />
              <Route path="/host" element={<HostSession />} />
              <Route path="/client" element={<ClientSession />} />
              <Route path="/settings" element={<Settings />} />
            </Routes>
          </main>
        </div>
      </div>
    </div>
  );
}

export default App;