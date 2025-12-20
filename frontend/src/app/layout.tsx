import type { Metadata } from "next";
import { Fraunces, Space_Grotesk } from "next/font/google";
import "./globals.css";

const spaceGrotesk = Space_Grotesk({
  variable: "--font-space-grotesk",
  subsets: ["latin"],
});

const fraunces = Fraunces({
  variable: "--font-fraunces",
  subsets: ["latin"],
  display: "swap",
});

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL;
const metadataBase = siteUrl ? new URL(siteUrl) : undefined;

export const metadata: Metadata = {
  title: "Sora Clean - Watermark Removal for Sora Videos",
  description:
    "Remove watermarks from Sora videos in minutes. Buy credits or subscribe for monthly removals.",
  metadataBase,
  openGraph: siteUrl
    ? {
      title: "Sora Clean - Watermark Removal",
      description:
        "Fast, secure watermark removal for Sora videos. Credits and monthly plans available.",
      url: siteUrl,
      siteName: "Sora Clean",
      type: "website",
    }
    : undefined,
  robots: {
    index: true,
    follow: true,
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${spaceGrotesk.variable} ${fraunces.variable} antialiased`}>
        {children}
      </body>
    </html>
  );
}
