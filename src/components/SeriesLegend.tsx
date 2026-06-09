export interface SeriesLegendItem<Key extends string = string> {
  key: Key;
  label: string;
  color: string;
  marker?: 'dot' | 'line';
}

export function SeriesLegend<Key extends string>({
  items,
  muted,
  onToggle,
}: {
  items: readonly SeriesLegendItem<Key>[];
  muted: Partial<Record<Key, boolean>>;
  onToggle: (key: Key) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-2 gap-y-2">
      {items.map((series) => {
        const isMuted = muted[series.key] ?? false;
        const isLine = series.marker === 'line';
        return (
          <button
            key={series.key}
            type="button"
            aria-pressed={!isMuted}
            title={isMuted ? `Show ${series.label}` : `Mute ${series.label}`}
            onClick={() => onToggle(series.key)}
            className={`flex items-center gap-1.5 rounded-md px-1.5 py-1 text-xs font-sans font-semibold transition-opacity hover:opacity-100 ${
              isMuted ? 'opacity-40' : 'opacity-100'
            }`}
          >
            <span
              className={`inline-block shrink-0 ${isLine ? 'w-5 h-0.5' : 'w-2.5 h-2.5 rounded-full'}`}
              style={{ backgroundColor: series.color }}
            />
            <span className="text-text-secondary">{series.label}</span>
          </button>
        );
      })}
    </div>
  );
}
