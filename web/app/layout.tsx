import type { Metadata } from "next";
import type { ReactNode } from "react";
import "./globals.css";

export const metadata: Metadata = {
  title: "SecondEgo — Local coding harness, bounded by design",
  description:
    "A local coding-agent harness where structured model plans meet deterministic policy, isolated Git worktrees, and real verification.",
};

export default function RootLayout({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-ink font-sans text-paper">{children}</body>
    </html>
  );
}
