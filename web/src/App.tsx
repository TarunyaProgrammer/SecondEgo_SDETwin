import { Nav } from "@/components/sections/Nav";
import { Hero } from "@/components/sections/Hero";
import { LogoMarqueeSection } from "@/components/sections/LogoMarquee";
import { Stats } from "@/components/sections/Stats";
import { JudgesMap } from "@/components/sections/JudgesMap";
import { ProofSection } from "@/components/sections/ProofSection";
import { LifecycleTabs } from "@/components/sections/LifecycleTabs";
import { FrameworkCards } from "@/components/sections/FrameworkCards";
import { ReferenceRunPanel } from "@/components/sections/ReferenceRunPanel";
import { TeamSection } from "@/components/sections/TeamSection";
import { FinalCTA } from "@/components/sections/FinalCTA";
import { Footer } from "@/components/sections/Footer";
import { ScrollExperience } from "@/components/ScrollExperience";
import "./styles.css";

export default function App() {
  return (
    <ScrollExperience>
      <Nav />
      <main id="main-content">
        <Hero />
        <LogoMarqueeSection />
        <Stats />
        <JudgesMap />
        <ProofSection />
        <LifecycleTabs />
        <FrameworkCards />
        <ReferenceRunPanel />
        <TeamSection />
        <FinalCTA />
      </main>
      <Footer />
    </ScrollExperience>
  );
}
