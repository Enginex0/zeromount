import "./Skeleton.css";

interface Props {
  width?: string;
  height?: string;
  borderRadius?: string;
  class?: string;
}

export function Skeleton(props: Props) {
  const styles = {
    "--skeleton-width": props.width || "100%",
    "--skeleton-height": props.height || "20px",
    "--skeleton-radius": props.borderRadius || "12px",
  } as Record<string, string | undefined>;

  return <div class={`skeleton ${props.class || ""}`} style={styles}></div>;
}
