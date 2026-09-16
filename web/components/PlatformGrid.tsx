"use client";

import { useEffect, useState } from "react";
import type { PlatformGroup, PlatformId } from "../lib/platform";
import { detectPlatform } from "../lib/platform";
import { ReleaseCard } from "./ReleaseCard";
import { ComingSoonCard } from "./ComingSoonCard";

export function PlatformGrid({ groups }: { groups: PlatformGroup[] }) {
  const [detected, setDetected] = useState<PlatformId | null>(null);

  useEffect(() => {
    setDetected(detectPlatform());
  }, []);

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
      {groups.map((group) =>
        group.available ? (
          <ReleaseCard key={group.id} group={group} recommended={detected === group.id} />
        ) : (
          <ComingSoonCard key={group.id} name={group.name} />
        )
      )}
    </div>
  );
}
