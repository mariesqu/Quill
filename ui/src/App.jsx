import { useState, useEffect } from 'react';
import MiniOverlay from './windows/MiniOverlay';
import FullPanel   from './windows/FullPanel';
import FirstRun    from './components/FirstRun';
import './styles/globals.css';

export default function App() {
  const [windowLabel, setWindowLabel] = useState(null);
  const [setupDone, setSetupDone] = useState(
    () => localStorage.getItem('quill_setup_complete') === 'true'
  );

  useEffect(() => {
    // Detect which Tauri window this webview belongs to
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => {
        setWindowLabel(getCurrentWindow().label);
      })
      .catch(() => setWindowLabel('mini'));
  }, []);

  // Apply stored theme on mount
  useEffect(() => {
    const theme = localStorage.getItem('quill_theme') || 'dark';
    document.documentElement.setAttribute('data-theme', theme);
  }, []);

  // Show first-run wizard if setup not complete
  if (!setupDone) {
    return (
      <FirstRun onComplete={() => {
        localStorage.setItem('quill_setup_complete', 'true');
        setSetupDone(true);
      }} />
    );
  }

  if (windowLabel === null) return null; // waiting for window label

  if (windowLabel === 'full') return <FullPanel />;
  return <MiniOverlay />;
}
