"use client";

import { useReducedMotion } from "framer-motion";

export type LogoWordmark = { name: string; mark?: string };

export function LogoMarquee({ items, label }: { items: LogoWordmark[]; label: string }) {
  const reduceMotion = useReducedMotion();
  const loopItems = [...items, ...items];

  return (
    <div className={`logo-marquee${reduceMotion ? " logo-marquee--static" : ""}`} aria-label={label}>
      <div className="logo-marquee__track">
        {loopItems.map((item, index) => (
          <span className="tech-wordmark" key={`${item.name}-${index}`} aria-hidden={index >= items.length}>
            {item.mark && <span className="tech-wordmark__mark" aria-hidden="true">{item.mark}</span>}{item.name}
          </span>
        ))}
      </div>
    </div>
  );
}
