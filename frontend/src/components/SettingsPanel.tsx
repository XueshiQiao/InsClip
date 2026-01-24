import { Settings } from '../types';
import { X, Save, Trash2, Info } from 'lucide-react';
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface SettingsPanelProps {
  settings: Settings;
  onClose: () => void;
  onSave: (settings: Settings) => void;
}

export function SettingsPanel({ settings: initialSettings, onClose, onSave }: SettingsPanelProps) {
  const [settings, setSettings] = useState<Settings>(initialSettings);
  const [historySize, setHistorySize] = useState<number>(0);

  useEffect(() => {
    invoke<number>('get_clipboard_history_size')
      .then(setHistorySize)
      .catch(console.error);
  }, []);

  const handleSave = () => {
    onSave(settings);
  };

  const handleClearHistory = async () => {
    try {
      await invoke('clear_clipboard_history');
      setHistorySize(0);
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
      <div className="bg-popover border border-border rounded-2xl w-full max-w-md mx-4 shadow-2xl animate-scale-in">
        <div className="flex items-center justify-between p-4 border-b border-border">
          <h2 className="text-lg font-semibold">Settings</h2>
          <button onClick={onClose} className="icon-button">
            <X size={18} />
          </button>
        </div>

        <div className="p-4 space-y-6">
          <div className="space-y-3">
            <label className="block">
              <span className="text-sm font-medium">Storage Limit</span>
              <span className="text-xs text-muted-foreground ml-2">({historySize} items stored)</span>
            </label>
            <input
              type="range"
              min="100"
              max="5000"
              step="100"
              value={settings.max_items}
              onChange={(e) => setSettings({ ...settings, max_items: parseInt(e.target.value) })}
              className="w-full accent-primary"
            />
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>100 items</span>
              <span className="font-medium text-primary">{settings.max_items} items</span>
              <span>5000 items</span>
            </div>
          </div>

          <div className="space-y-3">
            <label className="block">
              <span className="text-sm font-medium">Auto-delete after</span>
            </label>
            <select
              value={settings.auto_delete_days}
              onChange={(e) => setSettings({ ...settings, auto_delete_days: parseInt(e.target.value) })}
              className="w-full bg-input border border-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            >
              <option value="7">7 days</option>
              <option value="14">14 days</option>
              <option value="30">30 days</option>
              <option value="60">60 days</option>
              <option value="90">90 days</option>
              <option value="365">1 year</option>
              <option value="0">Never</option>
            </select>
          </div>

          <div className="space-y-3">
            <label className="block">
              <span className="text-sm font-medium">Hotkey</span>
            </label>
            <div className="flex items-center gap-2">
              <div className="flex-1 bg-input border border-border rounded-lg px-3 py-2 text-sm">
                {settings.hotkey}
              </div>
              <span className="text-xs text-muted-foreground">
                Press to change
              </span>
            </div>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <span className="text-sm font-medium">Startup with Windows</span>
              <p className="text-xs text-muted-foreground">
                Automatically start when Windows boots
              </p>
            </div>
            <button
              onClick={() => setSettings({ ...settings, startup_with_windows: !settings.startup_with_windows })}
              className={`w-11 h-6 rounded-full transition-colors ${
                settings.startup_with_windows ? 'bg-primary' : 'bg-accent'
              }`}
            >
              <div
                className={`w-5 h-5 rounded-full bg-white shadow-sm transition-transform ${
                  settings.startup_with_windows ? 'translate-x-5' : 'translate-x-0.5'
                }`}
              />
            </button>
          </div>

          <div className="pt-4 border-t border-border">
            <button
              onClick={handleClearHistory}
              className="btn btn-destructive w-full"
            >
              <Trash2 size={16} className="mr-2" />
              Clear All History
            </button>
          </div>

          <div className="flex items-start gap-2 p-3 rounded-lg bg-accent/50">
            <Info size={16} className="text-muted-foreground mt-0.5 flex-shrink-0" />
            <p className="text-xs text-muted-foreground">
              Items that are pinned will never be auto-deleted. Use the pin feature to keep important clips permanently.
            </p>
          </div>
        </div>

        <div className="flex items-center justify-end gap-2 p-4 border-t border-border">
          <button onClick={onClose} className="btn btn-secondary">
            Cancel
          </button>
          <button onClick={handleSave} className="btn btn-primary">
            <Save size={16} className="mr-2" />
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
