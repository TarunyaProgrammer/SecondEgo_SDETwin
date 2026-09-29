"use client";

import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

type ScrollExperienceProps = { children: ReactNode };

export function ScrollExperience({ children }: ScrollExperienceProps) {
  const [tone, setTone] = useState<"paper" | "ink">("paper");
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    const sections = Array.from(document.querySelectorAll<HTMLElement>("[data-tone]"));
    let frame = 0;

    const updateProgress = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const scrollable = document.documentElement.scrollHeight - window.innerHeight;
        setProgress(scrollable > 0 ? Math.min(1, Math.max(0, window.scrollY / scrollable)) : 0);
      });
    };

    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((entry) => entry.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
        const nextTone = visible?.target.getAttribute("data-tone");
        if (nextTone === "ink" || nextTone === "paper") setTone(nextTone);
      },
      { threshold: [0.2, 0.45, 0.7], rootMargin: "-16% 0px -52%" },
    );

    sections.forEach((section) => observer.observe(section));
    updateProgress();
    window.addEventListener("scroll", updateProgress, { passive: true });
    window.addEventListener("resize", updateProgress);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      window.removeEventListener("scroll", updateProgress);
      window.removeEventListener("resize", updateProgress);
    };
  }, []);

  const style = { "--scroll-progress": progress } as CSSProperties;

  return (
    <div className={`site-shell site-shell--${tone}`} style={style}>
      <div className="scroll-progress" aria-hidden="true"><span /></div>
      {children}
    </div>
  );
}
