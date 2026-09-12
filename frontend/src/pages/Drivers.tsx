// Drivers and firmware: one panel per device with its driver profiles as
// chwd reports them, and one panel for firmware as fwupd reports it. Every
// action starts a plan through startOps and is drawn disabled, saying why,
// while a plan already covers it.

import { Cpu, Microchip } from "lucide-react";
import { Fragment, useCallback, useEffect, useState, type ReactNode } from "react";
import { startOps } from "../activity/flow";
import * as api from "../api";
import { Badge, Button, Bytes, EmptyState, Figure, Notice, Panel, Skeleton } from "../components";
import { ICON_EMPTY, ICON_LG } from "../components/icons";
import type { DriverDevice, DriverProfile, DriversReport, FirmwareDevice, PackageRef } from "../types";
import { isBusy, useBusy, type Busy } from "./system/busy";
import "./system/system.css";

const NO_MANAGER = "Drivers are not managed on this distribution. chwd is the only driver manager BAP Store knows.";
const NO_FIRMWARE_SERVICE = "fwupd is not installed. Install the fwupd package to see firmware updates here.";
const NO_FIRMWARE_DEVICES = "fwupd found no device on this machine whose firmware it can update.";
const NO_DEVICES = "chwd found no device on this machine with a driver profile.";

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** "Installs mesa, lib32-mesa, vulkan-intel." */
function installsLine(packages: string[]): string {
  if (packages.length === 0) return "Installs no packages.";
  return `Installs ${packages.join(", ")}.`;
}

function ProfileRow({ profile, busy }: { profile: DriverProfile; busy: Busy }) {
  const ref: PackageRef = { source: "chwd", id: profile.id };
  const working = isBusy(busy, ref);
  const install = () => void startOps([{ op: "install", package: ref }], `Install driver profile ${profile.name}`);
  const remove = () => void startOps([{ op: "remove", package: ref }], `Remove driver profile ${profile.name}`);
  return (
    <div className="sy-profile">
      <div className="sy-profile-text">
        <div className={profile.installed ? "sy-profile-name on" : "sy-profile-name"}>
          {profile.installed ? (
            <span className="bs-dot bs-dot--accent" title="Installed" aria-hidden="true" />
          ) : (
            <span className="sy-dot-slot" aria-hidden="true" />
          )}
          <span className="bs-row-label">{profile.name}</span>
          <span className="bs-row-badges">
            {profile.installed ? <Badge tone="good">installed</Badge> : null}
            {profile.recommended ? <Badge title="chwd recommends this profile for the device.">recommended</Badge> : null}
          </span>
        </div>
        {profile.description ? <div className="sy-profile-desc">{profile.description}</div> : null}
        <div className="sy-profile-pkgs">{installsLine(profile.packages)}</div>
      </div>
      <div className="sy-profile-action">
        {profile.installed ? (
          working ? (
            <Button kind="ghost" disabled disabledReason="This profile is being removed. The activity panel shows its progress.">
              Removing…
            </Button>
          ) : (
            <Button kind="ghost" title={`Remove the ${profile.name} profile and its packages. You are asked to confirm first.`} onClick={remove}>
              Remove…
            </Button>
          )
        ) : working ? (
          <Button disabled disabledReason="This profile is being installed. The activity panel shows its progress.">
            Installing…
          </Button>
        ) : (
          <Button title={`Install the ${profile.name} profile with chwd.`} onClick={install}>
            Install
          </Button>
        )}
      </div>
    </div>
  );
}

function DevicePanel({ device, busy }: { device: DriverDevice; busy: Busy }) {
  const parts: ReactNode[] = [];
  if (device.vendor) parts.push(<span key="vendor">{device.vendor}</span>);
  if (device.class) parts.push(<span key="class">{device.class}</span>);
  parts.push(
    <Figure key="id" title="PCI address">
      {device.id}
    </Figure>,
  );
  const figure = (
    <span className="sy-panel-fig">
      {parts.map((part, i) => (
        <Fragment key={i}>
          {i > 0 ? <span aria-hidden="true">·</span> : null}
          {part}
        </Fragment>
      ))}
    </span>
  );
  return (
    <Panel title={device.name} icon={<Cpu {...ICON_LG} aria-hidden="true" />} commands={figure}>
      {device.profiles.length === 0 ? (
        <EmptyState>chwd has no profile for this device.</EmptyState>
      ) : (
        <div className="sy-profiles">
          {device.profiles.map((p) => (
            <ProfileRow key={p.id} profile={p} busy={busy} />
          ))}
        </div>
      )}
    </Panel>
  );
}

function FirmwareRow({ device, busy }: { device: FirmwareDevice; busy: Busy }) {
  const ref: PackageRef = { source: "fwupd", id: device.id };
  const working = isBusy(busy, ref);
  const update = () => void startOps([{ op: "update", package: ref }], `Update firmware for ${device.name}`);
  const hasUpdate = device.update_version !== null;
  return (
    <tr>
      <td>
        <div>{device.name}</div>
        {device.vendor ? <div className="sy-fw-sub">{device.vendor}</div> : null}
      </td>
      <td className="n">{device.version ? <Figure>{device.version}</Figure> : <span className="bs-dim">unknown</span>}</td>
      <td>
        {hasUpdate ? (
          <>
            <div className="sy-fw-update">
              <Figure>{device.update_version}</Figure>
              <Bytes value={device.update_size} />
              {device.needs_reboot ? <Badge tone="caution" title="The machine restarts to apply this update.">reboot</Badge> : null}
            </div>
            {device.update_summary ? <div className="sy-fw-sub">{device.update_summary}</div> : null}
          </>
        ) : (
          <span className="bs-dim">up to date</span>
        )}
      </td>
      <td className="sy-fw-action">
        {hasUpdate ? (
          working ? (
            <Button disabled disabledReason="This firmware is being updated. The activity panel shows its progress.">
              Updating…
            </Button>
          ) : (
            <Button title={`Update ${device.name} to ${device.update_version} with fwupd.`} onClick={update}>
              Update
            </Button>
          )
        ) : null}
      </td>
    </tr>
  );
}

function FirmwarePanel({ report, busy }: { report: DriversReport; busy: Busy }) {
  const updates = report.firmware.filter((d) => d.update_version !== null);
  const allBusy = busy.sources.has("fwupd");
  const updateAll = () => void startOps([{ op: "updateall", source: "fwupd" }], "Update all firmware");
  const commands =
    report.firmware_available && updates.length > 1 ? (
      allBusy ? (
        <Button disabled disabledReason="Firmware is being updated. The activity panel shows its progress.">
          Updating all firmware…
        </Button>
      ) : (
        <Button title={`Update ${updates.length} devices with fwupd.`} onClick={updateAll}>
          Update all firmware
        </Button>
      )
    ) : undefined;

  let body;
  if (!report.firmware_available) {
    body = <EmptyState icon={<Microchip {...ICON_EMPTY} aria-hidden="true" />}>{report.firmware_note ?? NO_FIRMWARE_SERVICE}</EmptyState>;
  } else if (report.firmware.length === 0) {
    body = <EmptyState icon={<Microchip {...ICON_EMPTY} aria-hidden="true" />}>{report.firmware_note ?? NO_FIRMWARE_DEVICES}</EmptyState>;
  } else {
    body = (
      <div className="bs-table-wrap">
        <table className="bs-table">
          <thead>
            <tr>
              <th scope="col">Device</th>
              <th scope="col" className="n">
                Version
              </th>
              <th scope="col">Available</th>
              <th scope="col">
                <span className="bs-sr">Action</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {report.firmware.map((d) => (
              <FirmwareRow key={d.id} device={d} busy={busy} />
            ))}
          </tbody>
        </table>
      </div>
    );
  }

  return (
    <Panel
      title="Firmware"
      icon={<Microchip {...ICON_LG} aria-hidden="true" />}
      count={report.firmware_available && report.firmware.length > 0 ? report.firmware.length : undefined}
      commands={commands}
    >
      {body}
    </Panel>
  );
}

/** A panel with three-line rows, the geometry of a device panel, while chwd and fwupd answer. */
function SkeletonPanel({ rows }: { rows: number }) {
  return (
    <div className="bs-panel" aria-hidden="true">
      <div className="sy-skel-head">
        <Skeleton width="28%" />
      </div>
      <div className="bs-panel-body">
        <div className="sy-profiles">
          {Array.from({ length: rows }, (_, i) => (
            <div key={i} className="sy-profile">
              <div className="sy-skel-lines">
                <Skeleton width={`${18 + ((i * 13) % 20)}%`} />
                <Skeleton width={`${50 + ((i * 23) % 30)}%`} className="bs-skel--text" />
                <Skeleton width={`${30 + ((i * 17) % 25)}%`} className="bs-skel--text" />
              </div>
              <Skeleton width="64px" height="26px" />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export function DriversPage() {
  const [report, setReport] = useState<DriversReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const busy = useBusy();

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setReport(await api.drivers());
    } catch (e) {
      setError(message(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const sub = report
    ? report.manager
      ? (report.manager_note ?? `Managed by ${report.manager}.`)
      : (report.manager_note ?? NO_MANAGER)
    : "Asking the machine which drivers and firmware it has.";

  const nothing = report !== null && report.manager === null && !report.firmware_available;

  return (
    <div className="bs-page" aria-busy={loading}>
      <div className="bs-page-head">
        <h1 className="bs-page-title">Drivers and firmware</h1>
        <p className="bs-page-sub">{sub}</p>
      </div>

      {error ? (
        <Notice
          actions={
            <Button kind="ghost" onClick={() => void load()}>
              Try again
            </Button>
          }
        >
          {error}
        </Notice>
      ) : null}

      {loading ? (
        <div aria-label="Loading" className="sy-skel-panels">
          <SkeletonPanel rows={2} />
          <SkeletonPanel rows={3} />
        </div>
      ) : null}

      {report && nothing ? (
        <EmptyState fill icon={<Cpu {...ICON_EMPTY} aria-hidden="true" />}>
          {report.manager_note ?? NO_MANAGER}
        </EmptyState>
      ) : null}

      {report && !nothing ? (
        <>
          {report.manager && report.devices.length === 0 ? (
            <div className="bs-well">
              <EmptyState icon={<Cpu {...ICON_EMPTY} aria-hidden="true" />}>{NO_DEVICES}</EmptyState>
            </div>
          ) : null}
          {report.devices.map((d) => (
            <DevicePanel key={d.id} device={d} busy={busy} />
          ))}
          <FirmwarePanel report={report} busy={busy} />
        </>
      ) : null}
    </div>
  );
}
