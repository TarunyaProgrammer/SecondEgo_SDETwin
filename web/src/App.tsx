import { useState } from "react";
import { Nav } from "@/components/sections/Nav";
import { Hero } from "@/components/sections/Hero";
import { LogoMarqueeSection } from "@/components/sections/LogoMarquee";
import { Stats } from "@/components/sections/Stats";
import { ProblemStatement } from "@/components/sections/ProblemStatement";
import { ArchitectureDiagram } from "@/components/sections/ArchitectureDiagram";
import { UserFlow } from "@/components/sections/UserFlow";
import { JudgesMap } from "@/components/sections/JudgesMap";
import { ProofSection } from "@/components/sections/ProofSection";
import { LifecycleTabs } from "@/components/sections/LifecycleTabs";
import { FrameworkCards } from "@/components/sections/FrameworkCards";
import { HonestBoundaries } from "@/components/sections/HonestBoundaries";
import { ReferenceRunPanel } from "@/components/sections/ReferenceRunPanel";
import { JudgeQA } from "@/components/sections/JudgeQA";
import { TeamSection } from "@/components/sections/TeamSection";
import { FinalCTA } from "@/components/sections/FinalCTA";
import { Footer } from "@/components/sections/Footer";
import { ScrollExperience } from "@/components/ScrollExperience";
import { PresentationModeOverlay, PresentButton } from "@/components/PresentationMode";
import "./styles.css";

export default function App() {
  const [presenting, setPresenting] = useState(false);

  return (
    <ScrollExperience>
      <Nav onPresent={() => setPresenting(true)} />
      <main id="main-content">
        <Hero />
        <LogoMarqueeSection />
        <Stats />
        <ProblemStatement />
        <ArchitectureDiagram />
        <UserFlow />
        <LifecycleTabs />
        <FrameworkCards />
        <JudgesMap />
        <ProofSection />
        <HonestBoundaries />
        <ReferenceRunPanel />
        <JudgeQA />
        <TeamSection />
        <FinalCTA />
      </main>
      <Footer />
      <PresentButton onClick={() => setPresenting(true)} />
      <PresentationModeOverlay isActive={presenting} onExit={() => setPresenting(false)} />
    </ScrollExperience>
  );
}
