import { Crumbs } from "./Bits";
import { JourneyEngine } from "./Engines";
import { Route } from "./Route";

/** The top of an inner page as one stop on the route: a spoken line that lands on the page's figure, or on the first `data-land` inside `list`. */
export function PageTop({
  say,
  short,
  crumbs,
  md,
  mark,
  cap,
  list,
  children,
}: {
  say: string;
  short?: string;
  crumbs?: [string, string][];
  md?: boolean;
  mark?: React.ReactNode;
  cap?: string;
  list?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <>
      <canvas className="weave" aria-hidden="true" />
      <section className="stop pt" data-intro="">
        <Route n={0} say={say} short={short} />
        <div className="wrap">
          {crumbs && <Crumbs items={crumbs} md={md} />}
          <div className={mark ? "pt-grid" : "pt-grid solo"}>
            <div className="pagehead">{children}</div>
            {mark && (
              <div className="pt-fig" data-life="">
                <div className="pt-mark" data-land="" data-at=".5">
                  {mark}
                </div>
                {cap && <p className="pt-cap">{cap}</p>}
              </div>
            )}
          </div>
          {list}
        </div>
      </section>
      <JourneyEngine />
    </>
  );
}
