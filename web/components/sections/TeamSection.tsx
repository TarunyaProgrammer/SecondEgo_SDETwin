"use client";

import { useState } from "react";
import { Reveal } from "@/components/Reveal";

const repository = "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin";

interface TeamMember {
  name: string;
  role: string;
  track: string;
  trackLabel: string;
  institution: string;
  bio: string;
  tags: string[];
  linkedin: string;
  github: string;
  handle: string;
  localImage: string;
  githubAvatar: string;
  initials: string;
  portfolio?: string;
}

const teamMembers: TeamMember[] = [
  {
    name: "Tarunya Kesharwani",
    role: "Runtime Architect & Systems Lead",
    track: "01",
    trackLabel: "SYSTEMS",
    institution: "Newton School of Technology",
    bio: "Engineered the core Rust execution engine, transactional worktree isolation, bounded model planning, and local verification loops.",
    tags: ["Rust Engine", "Bounded Runtimes", "GSoC '26", "Distributed Systems"],
    linkedin: "https://www.linkedin.com/in/tarunyakesharwani",
    github: "https://github.com/TarunyaProgrammer",
    handle: "TarunyaProgrammer",
    localImage: "/team/tarunya.jpg",
    githubAvatar: "https://avatars.githubusercontent.com/u/84562027?v=4",
    initials: "TK",
    portfolio: "https://tarunya.me",
  },
  {
    name: "Riya Agarwal",
    role: "Full-Stack & Systems Engineer",
    track: "02",
    trackLabel: "FULL-STACK",
    institution: "Newton School of Technology",
    bio: "Developed evaluation benchmarks, integration test harnesses, and end-to-end verification contracts across runtime boundaries.",
    tags: ["Full-Stack", "Evaluation Fixtures", "Test Harness", "Integration Contracts"],
    linkedin: "https://www.linkedin.com/in/riya5040agarwal",
    github: "https://github.com/riyaagarwal5040",
    handle: "riyaagarwal5040",
    localImage: "/team/riya.jpg",
    githubAvatar: "https://avatars.githubusercontent.com/u/226707053?v=4",
    initials: "RA",
  },
  {
    name: "Mansha Agarwal",
    role: "Frontend Engineer & Code Owner",
    track: "03",
    trackLabel: "CORE OWNER",
    institution: "Newton School of Technology",
    bio: "Spearheaded core interface modules, component architectures, and responsive state synchronization across engine tools.",
    tags: ["Frontend Systems", "Component Architecture", "Responsive UI", "Code Owner"],
    linkedin: "https://www.linkedin.com/in/mansha-agarwal-198759369",
    github: "https://github.com/ManshaAgarwal716",
    handle: "ManshaAgarwal716",
    localImage: "/team/mansha.jpg",
    githubAvatar: "https://avatars.githubusercontent.com/u/218825298?v=4",
    initials: "MA",
  },
  {
    name: "Yashika Gupta",
    role: "Frontend & Web Experience Engineer",
    track: "04",
    trackLabel: "WEB EXPERIENCE",
    institution: "Newton School of Technology",
    bio: "Crafted the visual presentation layer, typography hierarchy, micro-interactions, and accessibility standards for SecondEgo.",
    tags: ["Web Experience", "Design Systems", "Typography & Motion", "Accessibility"],
    linkedin: "https://www.linkedin.com/in/yashika-gupta-08781a2b7",
    github: "https://github.com/YashikaGupta2407",
    handle: "YashikaGupta2407",
    localImage: "/team/yashika.jpg",
    githubAvatar: "https://avatars.githubusercontent.com/u/222454612?v=4",
    initials: "YG",
  },
];

function MemberAvatar({ member }: { member: TeamMember }) {
  const [imgSrc, setImgSrc] = useState(member.localImage);
  const [hasError, setHasError] = useState(false);

  const handleError = () => {
    if (imgSrc === member.localImage && member.githubAvatar) {
      setImgSrc(member.githubAvatar);
    } else {
      setHasError(true);
    }
  };

  return (
    <div className="team-card__avatar-wrap">
      {!hasError ? (
        <img
          src={imgSrc}
          alt={member.name}
          className="team-card__avatar-img"
          onError={handleError}
          loading="lazy"
        />
      ) : (
        <span className="team-card__avatar-fallback" aria-hidden="true">
          {member.initials}
        </span>
      )}
      <span className="team-card__status-dot" title="Active Contributor" aria-hidden="true" />
    </div>
  );
}

export function TeamSection() {
  return (
    <section className="team-section wrap" id="team" data-tone="paper">
      <Reveal className="section-heading section-heading--split">
        <div>
          <p className="section-label">07 / CORE BUILDERS & AUTHORS</p>
          <h2>
            Built in the open.
            <br />
            <span className="text-accent">Held to proof.</span>
          </h2>
        </div>
        <div className="section-heading__aside-wrap">
          <p className="section-heading__aside">
            SecondEgo is built by engineers at Newton School of Technology dedicated to local-first, inspectable, and verifiable coding agents. Every commit and decision is open for inspection.
          </p>
          <div className="team-header-badges">
            <span className="team-badge-pill">
              <span className="team-badge-pill__dot" aria-hidden="true" /> 4 Engineers
            </span>
            <span className="team-badge-pill">
              <span className="team-badge-pill__dot" aria-hidden="true" /> Newton School of Tech
            </span>
            <span className="team-badge-pill">
              <span className="team-badge-pill__dot" aria-hidden="true" /> 100% Local-First
            </span>
            <span className="team-badge-pill">
              <span className="team-badge-pill__dot" aria-hidden="true" /> MIT Licensed
            </span>
          </div>
        </div>
      </Reveal>

      {/* 4-Card Team Grid */}
      <div className="team-cards-grid" aria-label="SecondEgo Core Team">
        {teamMembers.map((member, index) => (
          <Reveal key={member.name} className="team-card" delay={index * 0.08}>
            <div className="team-card__top">
              <MemberAvatar member={member} />
              <span className="team-card__track">
                #{member.track} · {member.trackLabel}
              </span>
            </div>

            <div className="team-card__content">
              <h3 className="team-card__name">{member.name}</h3>
              <p className="team-card__role">{member.role}</p>
              <p className="team-card__institution">{member.institution}</p>
              <p className="team-card__bio">{member.bio}</p>

              <div className="team-card__tags" aria-label="Areas of expertise">
                {member.tags.map((tag) => (
                  <span key={tag} className="team-card__tag-pill">
                    {tag}
                  </span>
                ))}
              </div>
            </div>

            <div className="team-card__actions">
              <a
                href={member.linkedin}
                target="_blank"
                rel="noreferrer"
                className="team-action-btn team-action-btn--linkedin"
                aria-label={`${member.name} on LinkedIn`}
              >
                <svg
                  className="team-icon"
                  viewBox="0 0 24 24"
                  width="13"
                  height="13"
                  fill="currentColor"
                  aria-hidden="true"
                >
                  <path d="M19 3a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h14m-.5 15.5v-5.3a3.26 3.26 0 0 0-3.26-3.26c-.85 0-1.84.52-2.28 1.3v-1.11h-2.79v8.37h2.79v-4.93c0-.77.62-1.4 1.39-1.4a1.4 1.4 0 0 1 1.4 1.4v4.93h2.75M6.46 10.9h2.79v8.37H6.46v-8.37M7.86 6.54a1.63 1.63 0 1 0 0 3.26 1.63 1.63 0 0 0 0-3.26z" />
                </svg>
                <span>LinkedIn</span>
                <span className="team-action-arrow" aria-hidden="true">↗</span>
              </a>

              <a
                href={member.github}
                target="_blank"
                rel="noreferrer"
                className="team-action-btn team-action-btn--github"
                aria-label={`${member.name} on GitHub`}
              >
                <svg
                  className="team-icon"
                  viewBox="0 0 24 24"
                  width="13"
                  height="13"
                  fill="currentColor"
                  aria-hidden="true"
                >
                  <path d="M12 2A10 10 0 0 0 2 12c0 4.42 2.87 8.17 6.84 9.5.5.08.66-.23.66-.5v-1.69c-2.77.6-3.36-1.34-3.36-1.34-.46-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.87 1.52 2.34 1.07 2.91.83.1-.65.35-1.09.63-1.34-2.22-.25-4.55-1.11-4.55-4.92 0-1.11.38-2 1.03-2.71-.1-.25-.45-1.29.1-2.64 0 0 .84-.27 2.75 1.02.79-.22 1.65-.33 2.5-.33.85 0 1.71.11 2.5.33 1.91-1.29 2.75-1.02 2.75-1.02.55 1.35.2 2.39.1 2.64.65.71 1.03 1.6 1.03 2.71 0 3.82-2.34 4.66-4.57 4.91.36.31.69.92.69 1.85V21c0 .27.16.59.67.5C19.14 20.16 22 16.42 22 12A10 10 0 0 0 12 2z" />
                </svg>
                <span>GitHub</span>
                <span className="team-action-arrow" aria-hidden="true">↗</span>
              </a>
            </div>
          </Reveal>
        ))}
      </div>

      {/* Open Source / Mission Statement Banner */}
      <Reveal className="team-philosophy-banner" delay={0.35}>
        <div className="team-philosophy-banner__left">
          <span className="team-philosophy-banner__mark" aria-hidden="true">SE</span>
          <p className="team-philosophy-banner__text">
            We are building the harness we wanted before trusting one with a real codebase: local by default, inspectable at every boundary, and honest about what has and has not been verified.
          </p>
        </div>
        <div className="team-philosophy-banner__links">
          <a
            className="text-link"
            href={`${repository}/blob/main/CONTRIBUTING.md`}
            target="_blank"
            rel="noreferrer"
          >
            How we work <span aria-hidden="true">↗</span>
          </a>
          <a
            className="text-link"
            href={`${repository}/blob/main/LICENSE`}
            target="_blank"
            rel="noreferrer"
          >
            MIT License <span aria-hidden="true">↗</span>
          </a>
          <a
            className="text-link"
            href={repository}
            target="_blank"
            rel="noreferrer"
          >
            GitHub Repo <span aria-hidden="true">↗</span>
          </a>
        </div>
      </Reveal>
    </section>
  );
}
