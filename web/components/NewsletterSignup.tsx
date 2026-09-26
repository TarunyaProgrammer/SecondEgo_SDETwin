"use client";

import { useState } from "react";
import type { FormEvent } from "react";

export function NewsletterSignup() {
  const [email, setEmail] = useState("");
  const [message, setMessage] = useState("");

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!email.trim()) return;
    setMessage("Signup is not connected yet. Follow repository updates on GitHub.");
    setEmail("");
  }

  return (
    <div className="newsletter-box">
      <p className="newsletter-box__title">Follow the build</p>
      <p>Product updates are shared through the repository for now.</p>
      <form onSubmit={handleSubmit} className="newsletter-form">
        <label className="sr-only" htmlFor="newsletter-email">Email address</label>
        <input id="newsletter-email" type="email" required value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@company.com" autoComplete="email" />
        <button type="submit" aria-label="Check update signup status">↗</button>
      </form>
      <p className="newsletter-box__message" aria-live="polite">{message || "Email signup is not connected."}</p>
    </div>
  );
}
