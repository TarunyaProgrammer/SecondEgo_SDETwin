import { useEffect, useRef, useState, useCallback } from "react";
import type { GestureActionType, GestureEvent, GestureState } from "../types";

const GESTURE_ACTION_MAP: Record<string, { action: GestureActionType; label: string }> = {
  thumbs_up: { action: "expand_notch", label: "👍 Expand Notch" },
  thumbs_down: { action: "collapse_notch", label: "👎 Collapse Notch" },
  fist: { action: "start_run", label: "✊ Start Run" },
  open_palm: { action: "cancel_run", label: "✋ Cancel / Pause" },
  point: { action: "focus_input", label: "☝ Focus Input" },
  two_fingers: { action: "scroll_up", label: "✌ Scroll Up" },
  three_fingers: { action: "scroll_down", label: "🤟 Scroll Down" },
};

interface UseGestureControlOptions {
  notchMode: boolean;
  gatewayUrl?: string;
  token?: string;
  onAction: (action: GestureActionType, event: GestureEvent) => void;
}

export function useGestureControl({
  notchMode,
  gatewayUrl,
  token,
  onAction,
}: UseGestureControlOptions): GestureState & {
  toggleEnabled: () => void;
  setEnabled: (val: boolean) => void;
  triggerManualGesture: (gesture: string) => void;
} {
  const [enabled, setEnabledState] = useState<boolean>(() => {
    if (!notchMode) return false;
    try {
      const stored = localStorage.getItem("secondego_notch_gestures");
      return stored !== null ? stored === "1" : true;
    } catch {
      return true;
    }
  });

  const [lastGesture, setLastGesture] = useState<GestureEvent | null>(null);
  const [cameraActive, setCameraActive] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const videoRef = useRef<HTMLVideoElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const animFrameRef = useRef<number | null>(null);
  const cooldownRef = useRef<{ [gesture: string]: number }>({});
  const debounceHistoryRef = useRef<string[]>([]);
  const lastFiredTimeRef = useRef<number>(0);

  const setEnabled = useCallback((val: boolean) => {
    setEnabledState(val);
    try {
      localStorage.setItem("secondego_notch_gestures", val ? "1" : "0");
    } catch {
      // ignore
    }
  }, []);

  const toggleEnabled = useCallback(() => {
    setEnabled(!enabled);
  }, [enabled, setEnabled]);

  // Dispatch an action with cooldown
  const dispatchGesture = useCallback(
    (gesture: string, confidence: number = 0.9, hand: string = "Right") => {
      const mapping = GESTURE_ACTION_MAP[gesture];
      if (!mapping) return;

      const now = Date.now();
      const lastForGesture = cooldownRef.current[gesture] || 0;
      // Cooldown of 1.4s between repeated same gestures, 0.6s between any gestures
      if (now - lastForGesture < 1400 || now - lastFiredTimeRef.current < 600) {
        return;
      }

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
      onAction(mapping.action, event);

      // Auto-clear gesture HUD after 2.5 seconds
      window.setTimeout(() => {
        setLastGesture((curr) => (curr?.timestamp === event.timestamp ? null : curr));
      }, 2500);
    },
    [onAction]
  );

  const triggerManualGesture = useCallback(
    (gesture: string) => {
      dispatchGesture(gesture, 1.0, "Manual");
    },
    [dispatchGesture]
  );

  // ── 1. Gateway SSE Listener (subscribes to Python/Rust GestureBroker) ──────
  useEffect(() => {
    if (!notchMode || !enabled || !gatewayUrl) return;

    let eventSource: EventSource | null = null;
    let cancelled = false;

    try {
      const url = new URL(`${gatewayUrl.replace(/\/$/, "")}/api/gestures/stream`);
      if (token) url.searchParams.set("token", token);

      eventSource = new EventSource(url.toString());

      eventSource.onmessage = (e) => {
        if (cancelled) return;
        try {
          const data = JSON.parse(e.data);
          if (data?.gesture) {
            dispatchGesture(data.gesture, data.confidence ?? 0.95, data.hand ?? "Right");
          }
        } catch {
          // ignore malformed frame
        }
      };

      eventSource.onerror = () => {
        // SSE endpoint might not be running if backend CV is optional; silent fallback
        eventSource?.close();
      };
    } catch {
      // ignore
    }

    return () => {
      cancelled = true;
      if (eventSource) {
        eventSource.close();
      }
    };
  }, [notchMode, enabled, gatewayUrl, token, dispatchGesture]);

  // ── 2. Native window.secondEgoWindow.onGesture IPC listener (Electron) ─────
  useEffect(() => {
    if (!notchMode || !enabled || typeof window === "undefined") return;
    if (!window.secondEgoWindow?.onGesture) return;

    const cleanup = window.secondEgoWindow.onGesture((event) => {
      dispatchGesture(event.gesture, event.confidence, event.hand);
    });

    return () => {
      cleanup?.();
    };
  }, [notchMode, enabled, dispatchGesture]);

  // ── 3. Client-side Camera Fallback (WebCam Vision when enabled) ────────────
  useEffect(() => {
    if (!notchMode || !enabled) {
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop());
        streamRef.current = null;
      }
      setCameraActive(false);
      return;
    }

    let isSubscribed = true;

    async function startCamera() {
      if (!navigator.mediaDevices?.getUserMedia) {
        return;
      }

      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          video: {
            width: { ideal: 320 },
            height: { ideal: 240 },
            frameRate: { ideal: 20 },
            facingMode: "user",
          },
          audio: false,
        });

        if (!isSubscribed) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }

        streamRef.current = stream;
        setCameraActive(true);
        setError(null);

        if (!videoRef.current) {
          const video = document.createElement("video");
          video.autoplay = true;
          video.playsInline = true;
          video.muted = true;
          video.style.display = "none";
          document.body.appendChild(video);
          videoRef.current = video;
        }

        videoRef.current.srcObject = stream;
        await videoRef.current.play().catch(() => {});

        if (!canvasRef.current) {
          const canvas = document.createElement("canvas");
          canvas.width = 160;
          canvas.height = 120;
          canvasRef.current = canvas;
        }

        // Lightweight optical gesture analyzer
        let frameCount = 0;
        const ctx = canvasRef.current.getContext("2d", { willReadFrequently: true });

        const processFrame = () => {
          if (!isSubscribed || !videoRef.current || !canvasRef.current || !ctx) return;

          frameCount++;
          // Sample every 4th frame (approx 5-6 FPS) to keep CPU near 0%
          if (frameCount % 4 === 0 && videoRef.current.readyState >= 2) {
            ctx.drawImage(videoRef.current, 0, 0, 160, 120);
            const imageData = ctx.getImageData(0, 0, 160, 120);
            const detected = analyzeOpticalHandGesture(imageData);

            if (detected) {
              debounceHistoryRef.current.push(detected);
              if (debounceHistoryRef.current.length > 5) {
                debounceHistoryRef.current.shift();
              }
              // If last 4 frames agree on the same gesture
              if (
                debounceHistoryRef.current.length >= 4 &&
                debounceHistoryRef.current.every((g) => g === detected)
              ) {
                dispatchGesture(detected, 0.85, "Camera");
              }
            } else {
              debounceHistoryRef.current = [];
            }
          }

          animFrameRef.current = requestAnimationFrame(processFrame);
        };

        animFrameRef.current = requestAnimationFrame(processFrame);
      } catch (err: unknown) {
        if (!isSubscribed) return;
        setCameraActive(false);
        const message = err instanceof Error ? err.message : "Camera permission unavailable";
        setError(message);
      }
    }

    void startCamera();

    return () => {
      isSubscribed = false;
      if (animFrameRef.current) {
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = null;
      }
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop());
        streamRef.current = null;
      }
      if (videoRef.current) {
        videoRef.current.pause();
        videoRef.current.remove();
        videoRef.current = null;
      }
      setCameraActive(false);
    };
  }, [notchMode, enabled, dispatchGesture]);

  return {
    enabled: notchMode && enabled,
    active: notchMode && enabled && cameraActive,
    lastGesture,
    cameraActive: notchMode && cameraActive,
    error: notchMode ? error : null,
    toggleEnabled,
    setEnabled,
    triggerManualGesture,
  };
}

/**
 * Lightweight, zero-dependency optical skin-color + convex contour analyzer.
 * Runs on 160x120 downsampled buffer with negligible CPU footprint.
 */
function analyzeOpticalHandGesture(imageData: ImageData): string | null {
  const { data, width, height } = imageData;
  let skinPixels = 0;
  let minX = width;
  let maxX = 0;
  let minY = height;
  let maxY = 0;
  let sumX = 0;
  let sumY = 0;

  // YCbCr / normalized RGB skin color filter
  for (let i = 0; i < data.length; i += 4) {
    const r = data[i];
    const g = data[i + 1];
    const b = data[i + 2];

    // Standard skin chrominance heuristic
    if (r > 95 && g > 40 && b > 20 && r > g && r > b && r - g > 15 && Math.abs(r - g) > 15) {
      const pixelIdx = i / 4;
      const x = pixelIdx % width;
      const y = Math.floor(pixelIdx / width);

      skinPixels++;
      sumX += x;
      sumY += y;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
  }

  // Minimum hand size threshold: at least ~4% of image area
  const totalPixels = width * height;
  if (skinPixels < totalPixels * 0.04) {
    return null;
  }

  const bboxWidth = maxX - minX;
  const bboxHeight = maxY - minY;
  if (bboxWidth <= 0 || bboxHeight <= 0) return null;

  const aspectRatio = bboxHeight / bboxWidth;
  const density = skinPixels / (bboxWidth * bboxHeight);
  const centerY = sumY / skinPixels;
  const relativeCenterY = (centerY - minY) / bboxHeight;

  // Heuristic gesture classification based on contour geometry:
  // 1. Thumbs Up: Tall, top-heavy thumb extension, moderate density
  if (aspectRatio > 1.4 && relativeCenterY > 0.58 && density < 0.55) {
    return "thumbs_up";
  }

  // 2. Thumbs Down: Tall, bottom-heavy thumb extension
  if (aspectRatio > 1.4 && relativeCenterY < 0.42 && density < 0.55) {
    return "thumbs_down";
  }

  // 3. Fist: Compact, high bounding-box fill density, square-ish aspect ratio
  if (density > 0.65 && aspectRatio >= 0.8 && aspectRatio <= 1.35) {
    return "fist";
  }

  // 4. Open Palm: Large skin area, lower density due to finger gaps, tall or wide
  if (density < 0.45 && skinPixels > totalPixels * 0.08) {
    return "open_palm";
  }

  // 5. Point: Vertical slender extension with palm base
  if (aspectRatio > 1.5 && density < 0.5) {
    return "point";
  }

  // 6. Two fingers (Peace / V): Moderate aspect ratio with distinct split
  if (aspectRatio > 1.2 && density < 0.48) {
    return "two_fingers";
  }

  return null;
}
