import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { ViewTransition } from "react";
import { CopyEngine } from "@/components/Engines";
import { Footer } from "@/components/Footer";
import { Header } from "@/components/Header";
import { SITE_URL } from "@/lib/site";
import "./globals.css";

const inter = localFont({ src: "./fonts/inter-display.woff2", weight: "400", variable: "--font-inter", display: "swap" });
const jetbrains = localFont({ src: "./fonts/jetbrains-mono.woff2", weight: "100 800", variable: "--font-jb", display: "swap", adjustFontFallback: false });

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  icons: { icon: { url: "/favicon.svg", type: "image/svg+xml" } },
};

export const viewport: Viewport = {
  colorScheme: "light dark",
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#ffffff" },
    { media: "(prefers-color-scheme: dark)", color: "#141412" },
  ],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" data-scroll-behavior="smooth" className={`${inter.variable} ${jetbrains.variable}`}>
      <body>
        <a className="skip" href="#main">
          Skip to content
        </a>
        <Header />
        <ViewTransition default="vt-page">
          <main id="main">{children}</main>
        </ViewTransition>
        <Footer />
        <CopyEngine />
      </body>
    </html>
  );
}
