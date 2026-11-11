"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { NAV, REPO } from "@/lib/site";
import stars from "@/lib/stars.json";
import { Star } from "./Bits";
import { Wordmark } from "./Wordmark";

export function Header() {
  const path = usePathname();
  const count = stars.stars < 1000 ? stars.stars : `${(stars.stars / 1000).toFixed(1)}k`;
  const shown = stars.stars > 10;
  return (
    <header className={path === "/" ? "top over" : "top"}>
      <div className="wrap bar">
        <Link className="brand" href="/" aria-label="phonedriver home">
          <Wordmark />
        </Link>
        <nav className="nav" aria-label="Main">
          {NAV.map((n) => (
            <Link key={n.href} href={n.href} aria-current={path.startsWith(n.href) ? "page" : undefined}>
              {n.label}
            </Link>
          ))}
          <a className="gh" href={REPO}>
            <Star />
            GitHub
            {shown && (
              <>
                {" "}
                <b>{count}</b>
                <span className="vh"> stars</span>
              </>
            )}
          </a>
          <button type="button" className="menu-btn" popoverTarget="menu">
            Menu
          </button>
        </nav>
      </div>
      <div className="menu" id="menu" popover="auto">
        <p className="menu-top">
          <span>phonedriver</span>
          <button type="button" popoverTarget="menu" popoverTargetAction="hide">
            Close
          </button>
        </p>
        <nav aria-label="All pages" onClick={(e) => (e.target as HTMLElement).closest("a") && document.getElementById("menu")?.hidePopover()}>
          {NAV.map((n) => (
            <Link key={n.href} href={n.href} aria-current={path.startsWith(n.href) ? "page" : undefined}>
              {n.label}
            </Link>
          ))}
          <a href={REPO}>
            GitHub
            {shown && <span>{count} stars</span>}
          </a>
        </nav>
      </div>
    </header>
  );
}
