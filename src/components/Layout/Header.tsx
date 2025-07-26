import { Menu, Sun, Moon, Monitor } from "lucide-react";
import { useTheme } from "@hooks/useTheme";

interface HeaderProps {
  onMenuClick: () => void;
  systemInfo?: any;
}

export default function Header({ onMenuClick, systemInfo }: HeaderProps) {
  const { theme, setTheme } = useTheme();

  const toggleTheme = () => {
    if (theme === "light") {
      setTheme("dark");
    } else if (theme === "dark") {
      setTheme("system");
    } else {
      setTheme("light");
    }
  };

  const getThemeIcon = () => {
    switch (theme) {
      case "light":
        return <Sun className="w-5 h-5" />;
      case "dark":
        return <Moon className="w-5 h-5" />;
      default:
        return <Monitor className="w-5 h-5" />;
    }
  };

  return (
    <header className="bg-white dark:bg-gray-800 shadow border-b border-gray-200 dark:border-gray-700">
      <div className="px-4 sm:px-6 lg:px-8">
        <div className="flex justify-between h-16">
          <div className="flex items-center">
            <button
              type="button"
              className="lg:hidden p-2 rounded-md text-gray-400 hover:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-primary-500"
              onClick={onMenuClick}
            >
              <Menu className="h-6 w-6" />
            </button>
            
            <div className="hidden lg:flex lg:items-center lg:ml-4">
              <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
                Remote Desktop
              </h2>
            </div>
          </div>

          <div className="flex items-center space-x-4">
            {/* System Info */}
            {systemInfo && (
              <div className="hidden md:flex items-center space-x-2 text-sm text-gray-500 dark:text-gray-400">
                <span>{systemInfo.os}</span>
                <span>•</span>
                <span>{systemInfo.hostname}</span>
              </div>
            )}

            {/* Theme Toggle */}
            <button
              onClick={toggleTheme}
              className="p-2 rounded-md text-gray-400 hover:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-primary-500"
              title={`Current theme: ${theme}`}
            >
              {getThemeIcon()}
            </button>

            {/* User Menu */}
            <div className="flex items-center">
              <div className="w-8 h-8 bg-primary-600 rounded-full flex items-center justify-center">
                <span className="text-sm font-medium text-white">
                  {systemInfo?.username?.charAt(0)?.toUpperCase() || "U"}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}