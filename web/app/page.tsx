import { Nav } from "@/components/sections/Nav";
import { Hero } from "@/components/sections/Hero";
import { LogoMarqueeSection } from "@/components/sections/LogoMarquee";
import { Stats } from "@/components/sections/Stats";
import { LifecycleTabs } from "@/components/sections/LifecycleTabs";
import { FrameworkCards } from "@/components/sections/FrameworkCards";
import { ReferenceRunPanel } from "@/components/sections/ReferenceRunPanel";
import { FinalCTA } from "@/components/sections/FinalCTA";
import { Footer } from "@/components/sections/Footer";

export default function HomePage() {
  return (
    <>
      <Nav />
      <main>
        <Hero />
        <LogoMarqueeSection />
        <Stats />
        <LifecycleTabs />
        <FrameworkCards />
        <ReferenceRunPanel />
        <FinalCTA />
      </main>
      <Footer />
    </>
  );
}
