import { useState } from "react";
import type { GestureEvent, GestureServiceState } from "../types";

interface GestureIndicatorProps {
  enabled: boolean;
  lastGesture: GestureEvent | null;
  cameraActive: boolean;
  error: string | null;
  serviceState: GestureServiceState;
  onToggle: () => void;
  onTriggerManual?: (gesture: string) => void;
  onOpenNotch?: () => void;
}

export function GestureIndicator({
  enabled,
  lastGesture,
  cameraActive,
  error,
  serviceState,
  onToggle,
  onTriggerManual,
  onOpenNotch,
}: GestureIndicatorProps) {
  const [showHelp, setShowHelp] = useState(false);
  const needsRetry = enabled && (serviceState === "unavailable" || serviceState === "error");
  const trackerLabel = cameraActive
    ? "Landmarks Live"
    : serviceState === "starting"
      ? "Starting tracker"
      : needsRetry
        ? "Retry tracker"
        : "Gestures Off";
  const toggleTitle = needsRetry
    ? `${error || "Landmark tracking is unavailable."} Click to retry.`
    : enabled
      ? "Landmark tracking is on. Click to turn it off."
      : "Turn on local landmark tracking.";

  return (
    <div className="gesture-indicator-wrapper" role="region" aria-label="Notch hand gesture control">
      {/* Toast banner when a gesture is recognized */}
      {lastGesture && (
        <div className="gesture-toast animate-toast" role="status" aria-live="polite">
          <span className="gesture-toast-icon" aria-hidden="true" />
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
          title={toggleTitle}
          aria-label={needsRetry ? "Retry local landmark tracking" : enabled ? "Disable hand gesture controls" : "Enable hand gesture controls"}
        >
          <span className={`gesture-status-dot ${cameraActive ? "is-live" : enabled ? "is-standby" : "is-off"}`} />
          <svg className="gesture-cam-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
            <circle cx="12" cy="13" r="4" />
          </svg>
          <span className="gesture-text">
            {trackerLabel}
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
          <p className="gesture-help-desc">Hold one gesture steady for about a second. Local hand landmarks only change the companion window; run controls stay explicit.</p>
          <ul className="gesture-list">
            <li><button type="button" onClick={() => onTriggerManual?.("thumbs_up")}><span className="g-icon">UP</span><span><b>Thumbs up</b><small>Expand mission control</small></span></button></li>
            <li><button type="button" onClick={() => onTriggerManual?.("thumbs_down")}><span className="g-icon">DN</span><span><b>Thumbs down</b><small>Collapse the companion</small></span></button></li>
            <li><button type="button" onClick={() => onTriggerManual?.("open_palm")}><span className="g-icon">PALM</span><span><b>Open palm</b><small>Fallback to expand mission control</small></span></button></li>
          </ul>
          {error && <p className="gesture-error-note">{error}</p>}
        </div>
      )}
    </div>
  );
}
