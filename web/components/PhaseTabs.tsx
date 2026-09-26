"use client";

export type PhaseTabItem = { id: string; label: string; summary: string };

type PhaseTabsProps = {
  items: PhaseTabItem[];
  activeId: string;
  onChange: (id: string) => void;
};

export function PhaseTabs({ items, activeId, onChange }: PhaseTabsProps) {
  return (
    <div className="phase-tabs" role="tablist" aria-label="Execution lifecycle">
      {items.map((item, index) => {
        const active = activeId === item.id;
        return (
          <button
            type="button"
            key={item.id}
            id={`tab-${item.id}`}
            role="tab"
            aria-selected={active}
            aria-controls={`panel-${item.id}`}
            tabIndex={active ? 0 : -1}
            className={`phase-tabs__item${active ? " is-active" : ""}`}
            onClick={() => onChange(item.id)}
            onKeyDown={(event) => {
              const currentIndex = items.findIndex((tab) => tab.id === item.id);
              const forward = event.key === "ArrowRight" || event.key === "ArrowDown";
              const backward = event.key === "ArrowLeft" || event.key === "ArrowUp";
              const nextIndex = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : forward ? (currentIndex + 1) % items.length : backward ? (currentIndex - 1 + items.length) % items.length : -1;
              if (nextIndex >= 0) {
                event.preventDefault();
                onChange(items[nextIndex].id);
                requestAnimationFrame(() => document.getElementById(`tab-${items[nextIndex].id}`)?.focus());
              }
            }}
          >
            <span className="phase-tabs__index">0{index + 1}</span>
            <span className="phase-tabs__copy"><span className="phase-tabs__label">{item.label}</span><span className="phase-tabs__summary">{item.summary}</span></span>
            <span className="phase-tabs__arrow" aria-hidden="true">↗</span>
          </button>
        );
      })}
    </div>
  );
}
