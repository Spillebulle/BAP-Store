interface Props {
  /** CSS width; the geometry of the content it stands in for. */
  width?: string;
  height?: string;
  className?: string;
}

/** A control block with the slow shimmer, same geometry as what is coming (§7.18). */
export function Skeleton({ width, height, className }: Props) {
  return <span className={className ? `bs-skel ${className}` : "bs-skel"} style={{ width, height }} aria-hidden="true" />;
}

/** Application rows while a search is running: icon, name, summary. */
export function SkeletonAppRows({ count = 6 }: { count?: number }) {
  return (
    <div className="bs-list" aria-busy="true" aria-label="Loading">
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="bs-row bs-row--app">
          <Skeleton className="bs-skel--icon" />
          <div className="bs-row-text" style={{ gap: "6px" }}>
            <Skeleton width={`${28 + ((i * 17) % 30)}%`} />
            <Skeleton width={`${45 + ((i * 23) % 40)}%`} className="bs-skel--text" />
          </div>
        </div>
      ))}
    </div>
  );
}
