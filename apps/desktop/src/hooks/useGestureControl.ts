import { useCallback, useEffect, useRef, useState } from "react";
import type {
  GestureActionType,
  GestureEvent,
  GestureServiceState,
  GestureServiceStatus,
  GestureState,
  LandmarkGestureSignal,
} from "../types";

const GESTURE_ACTION_MAP: Record<string, { action: GestureActionType; label: string }> = {
  thumbs_up: { action: "expand_notch", label: "Thumbs up · Expand notch" },
  thumbs_down: { action: "collapse_notch", label: "Thumbs down · Collapse notch" },
  open_palm: { action: "expand_notch", label: "Open palm · Expand notch" },
};

const gestureServiceStates = new Set<GestureServiceState>([
  "disabled",
  "starting",
  "active",
  "unavailable",
  "error",
]);

interface UseGestureControlOptions {
  notchMode: boolean;
  onAction: (action: GestureActionType, event: GestureEvent) => void;
}

function serviceStatus(state: GestureServiceState, message: string | null = null): GestureServiceStatus {
  return {
    schema_version: 1,
    state,
    message,
    timestamp: new Date().toISOString(),
  };
}

function isGestureStatus(value: unknown): value is GestureServiceStatus {
  if (!value || typeof value !== "object") return false;
  const status = value as Partial<GestureServiceStatus>;
  return status.schema_version === 1
    && typeof status.state === "string"
    && gestureServiceStates.has(status.state as GestureServiceState)
    && (status.message === null || typeof status.message === "string")
    && typeof status.timestamp === "string";
}

function isLandmarkSignal(value: unknown): value is LandmarkGestureSignal {
  if (!value || typeof value !== "object") return false;
  const signal = value as Partial<LandmarkGestureSignal>;
  return signal.schema_version === 1
    && (signal.gesture === "thumbs_up" || signal.gesture === "thumbs_down" || signal.gesture === "open_palm")
    && (signal.hand === "Left" || signal.hand === "Right")
    && typeof signal.confidence === "number"
    && typeof signal.timestamp === "string";
}

export function useGestureControl({
  notchMode,
  onAction,
}: UseGestureControlOptions): GestureState & {
  toggleEnabled: () => void;
  setEnabled: (val: boolean) => void;
  triggerManualGesture: (gesture: string) => void;
} {
  const [enabled, setEnabledState] = useState<boolean>(() => {
    if (!notchMode) return false;
    try {
      const requested = new URLSearchParams(window.location.search).get("gestures");
      if (requested === "1") return true;
      if (requested === "0") return false;
      const stored = localStorage.getItem("secondego_notch_gestures");
      return stored !== null ? stored === "1" : false;
    } catch {
      return false;
    }
  });
  const [lastGesture, setLastGesture] = useState<GestureEvent | null>(null);
  const [trackerStatus, setTrackerStatus] = useState<GestureServiceStatus>(() => serviceStatus("disabled"));

  const audioContextRef = useRef<AudioContext | null>(null);
  const cooldownRef = useRef<Record<string, number>>({});
  const lastFiredTimeRef = useRef(0);
  const onActionRef = useRef(onAction);

  const ensureAudioContext = useCallback(() => {
    const AudioContextConstructor = window.AudioContext
      || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioContextConstructor) return null;
    if (!audioContextRef.current) audioContextRef.current = new AudioContextConstructor();
    return audioContextRef.current;
  }, []);

  const primeGestureAudio = useCallback(async () => {
    const context = ensureAudioContext();
    if (context?.state === "suspended") await context.resume().catch(() => {});
  }, [ensureAudioContext]);

  const playGestureChime = useCallback(() => {
    const context = ensureAudioContext();
    if (!context) return;
    const play = () => {
      const now = context.currentTime;
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = "sine";
      oscillator.frequency.setValueAtTime(880, now);
      oscillator.frequency.exponentialRampToValueAtTime(1320, now + 0.09);
      gain.gain.setValueAtTime(0.0001, now);
      gain.gain.setValueAtTime(0.08, now + 0.012);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.16);
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.start(now);
      oscillator.stop(now + 0.17);
    };
    if (context.state === "suspended") {
      void context.resume().then(play).catch(() => {});
    } else {
      play();
    }
  }, [ensureAudioContext]);

  useEffect(() => {
    onActionRef.current = onAction;
  }, [onAction]);

  const setEnabled = useCallback((value: boolean) => {
    setEnabledState(value);
    if (value) void primeGestureAudio();
    try {
      localStorage.setItem("secondego_notch_gestures", value ? "1" : "0");
    } catch {
      // Storage is a convenience only; controls remain click-first.
    }
  }, [primeGestureAudio]);

  const toggleEnabled = useCallback(() => {
    if (enabled && (trackerStatus.state === "unavailable" || trackerStatus.state === "error")) {
      const setHostGestureEnabled = window.secondEgoWindow?.setGestureEnabled;
      if (!setHostGestureEnabled) return;
      setTrackerStatus(serviceStatus("starting"));
      void setHostGestureEnabled(false)
        .then(() => setHostGestureEnabled(true))
        .then((status) => {
          if (isGestureStatus(status)) setTrackerStatus(status);
        })
        .catch(() => setTrackerStatus(serviceStatus("error", "Could not restart local landmark tracking.")));
      return;
    }
    setEnabled(!enabled);
  }, [enabled, setEnabled, trackerStatus.state]);

  const dispatchGesture = useCallback((gesture: string, confidence = 1, hand = "Right") => {
    const mapping = GESTURE_ACTION_MAP[gesture];
    if (!mapping) return;

    const now = Date.now();
    const lastForGesture = cooldownRef.current[gesture] || 0;
    if (now - lastForGesture < 1400 || now - lastFiredTimeRef.current < 600) return;

    cooldownRef.current[gesture] = now;
    lastFiredTimeRef.current = now;
    const event: GestureEvent = {
      gesture,
      action: mapping.action,
      confidence,
      label: mapping.label,
      hand,
      timestamp: new Date().toISOString(),
    };
    setLastGesture(event);
    playGestureChime();
    onActionRef.current(mapping.action, event);
    window.setTimeout(() => {
      setLastGesture((current) => current?.timestamp === event.timestamp ? null : current);
    }, 2500);
  }, [playGestureChime]);

  const triggerManualGesture = useCallback((gesture: string) => {
    if (enabled) dispatchGesture(gesture, 1, "Manual");
  }, [dispatchGesture, enabled]);

  // The host owns camera access and invokes the existing Python MediaPipe
  // watcher. The renderer deliberately has no getUserMedia fallback: color and
  // contour guesses caused the broken thumbs-up behavior this replaces.
  useEffect(() => {
    if (!notchMode) return;
    const bridge = window.secondEgoWindow;
    if (!bridge?.setGestureEnabled) {
      setTrackerStatus(enabled
        ? serviceStatus("unavailable", "Landmark gestures are available in the Electron companion.")
        : serviceStatus("disabled"));
      return;
    }

    let current = true;
    void bridge.setGestureEnabled(enabled)
      .then((status) => {
        if (current && isGestureStatus(status)) setTrackerStatus(status);
      })
      .catch(() => {
        if (current) setTrackerStatus(serviceStatus("error", "Could not configure local landmark tracking."));
      });
    return () => { current = false; };
  }, [enabled, notchMode]);

  useEffect(() => {
    if (!notchMode) return;
    const unsubscribe = window.secondEgoWindow?.onGestureStatus?.((status) => {
      if (isGestureStatus(status)) setTrackerStatus(status);
    });
    return () => unsubscribe?.();
  }, [notchMode]);

  useEffect(() => {
    if (!notchMode || !enabled) return;
    const unsubscribe = window.secondEgoWindow?.onGesture?.((signal) => {
      if (isLandmarkSignal(signal)) {
        dispatchGesture(signal.gesture, signal.confidence, signal.hand);
      }
    });
    return () => unsubscribe?.();
  }, [dispatchGesture, enabled, notchMode]);

  const cameraActive = notchMode && enabled && trackerStatus.state === "active";
  const error = notchMode && (trackerStatus.state === "unavailable" || trackerStatus.state === "error")
    ? trackerStatus.message || "Landmark tracking is unavailable."
    : null;

  return {
    enabled: notchMode && enabled,
    active: cameraActive,
    lastGesture,
    cameraActive,
    error,
    serviceState: notchMode ? trackerStatus.state : "disabled",
    serviceMessage: notchMode ? trackerStatus.message : null,
    toggleEnabled,
    setEnabled,
    triggerManualGesture,
  };
}
