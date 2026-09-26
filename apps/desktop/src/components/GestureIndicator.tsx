import { useState } from "react";
import type { GestureEvent } from "../types";

interface GestureIndicatorProps {
  enabled: boolean;
  active: boolean;
  lastGesture: GestureEvent | null;
  cameraActive: boolean;
  error: string | null;
  onToggle: () => void;
  onTriggerManual?: (gesture: string) => void;
  onOpenNotch?: () => void;
}

export function GestureIndicator({
  enabled,
  active,
  lastGesture,
  cameraActive,
  error,
  onToggle,
  onTriggerManual,
  onOpenNotch,
}: GestureIndicatorProps) {
  const [showHelp, setShowHelp] = useState(false);

  return (
    <div className="gesture-indicator-wrapper" role="region" aria-label="Notch hand gesture control">
      {/* Toast banner when a gesture is recognized */}
      {lastGesture && (
        <div className="gesture-toast animate-toast" role="status" aria-live="polite">
          <span className="gesture-toast-icon">✨</span>
          <span className="gesture-toast-label">{lastGesture.label}</span>
          <span className="gesture-toast-badge">{lastGesture.hand || "Camera"}</span>
        </div>
      )}

      {/* Main trigger pill / toggle */}
      <div className={`gesture-pill ${enabled ? "is-enabled" : "is-disabled"} ${cameraActive ? "is-active" : ""}`}>
        <button
          type="button"
          className="gesture-toggle-btn"
          onClick={onToggle}
          title={
            enabled
              ? "Gesture control is ON (Click to disable camera gestures)"
              : "Gesture control is OFF (Click to enable camera gestures)"
          }
          aria-label={enabled ? "Disable hand gesture controls" : "Enable hand gesture controls"}
        >
          <span className={`gesture-status-dot ${cameraActive ? "is-live" : enabled ? "is-standby" : "is-off"}`} />
          <svg className="gesture-cam-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
            <circle cx="12" cy="13" r="4" />
          </svg>
          <span className="gesture-text">
            {cameraActive ? "Gestures Live" : enabled ? "Gestures Ready" : "Gestures Off"}
          </span>
        </button>

        <button
          type="button"
          className="gesture-help-btn"
          onClick={(e) => {
            e.stopPropagation();
            onOpenNotch?.();
            setShowHelp((h) => !h);
          }}
          title="Show gesture guide"
          aria-label="Show gesture cheat sheet"
        >
          ?
        </button>
      </div>

      {/* Gesture Help Dropdown Modal */}
      {showHelp && (
        <div className="gesture-help-card animate-card" onClick={(e) => e.stopPropagation()}>
          <header className="gesture-help-header">
            <h4>Hand Gesture Controls</h4>
            <button type="button" className="gesture-close-btn" onClick={() => setShowHelp(false)}>×</button>
          </header>
          <p className="gesture-help-desc">Hold hand clearly in camera view to trigger commands:</p>
          <ul className="gesture-list">
            <li onClick={() => onTriggerManual?.("thumbs_up")}>
              <span className="g-icon">👍</span>
              <div><b>Thumbs Up</b><small>Expand notch mission control</small></div>
            </li>
            <li onClick={() => onTriggerManual?.("thumbs_down")}>
              <span className="g-icon">👎</span>
              <div><b>Thumbs Down</b><small>Collapse into notch</small></div>
            </li>
            <li onClick={() => onTriggerManual?.("fist")}>
              <span className="g-icon">✊</span>
              <div><b>Fist</b><small>Start verified agent run</small></div>
            </li>
            <li onClick={() => onTriggerManual?.("open_palm")}>
              <span className="g-icon">✋</span>
              <div><b>Open Palm</b><small>Stop or cancel run</small></div>
            </li>
            <li onClick={() => onTriggerManual?.("point")}>
              <span className="g-icon">☝</span>
              <div><b>Point</b><small>Focus repository / task input</small></div>
            </li>
          </ul>
          {error && <p className="gesture-error-note">{error}</p>}
        </div>
      )}
    </div>
  );
}
