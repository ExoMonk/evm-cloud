/**
 * ABI registry — maps ABI filenames to their JSON content.
 * Imported directly from template source files.
 */

import ERC20 from "./abis/ERC20.json";
import ERC721 from "./abis/ERC721.json";
import PoolManager from "./abis/PoolManager.json";
import AaveV3Pool from "./abis/AaveV3Pool.json";
import AaveV4Spoke from "./abis/AaveV4Spoke.json";

const ABI_MAP: Record<string, unknown[]> = {
  "ERC20.json": ERC20,
  "ERC721.json": ERC721,
  "PoolManager.json": PoolManager,
  "AaveV3Pool.json": AaveV3Pool,
  "AaveV4Spoke.json": AaveV4Spoke,
};

/** Get ABI JSON string for a given filename */
export function getAbiContent(filename: string): string {
  const abi = ABI_MAP[filename];
  if (!abi) return "[]";
  return JSON.stringify(abi, null, 2);
}

/** Get all ABI files for a template (by ABI filenames) */
export function getAbisForTemplate(abiFilenames: string[]): Record<string, string> {
  const result: Record<string, string> = {};
  for (const filename of abiFilenames) {
    result[filename] = getAbiContent(filename);
  }
  return result;
}
