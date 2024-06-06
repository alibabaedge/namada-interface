import {
  DefaultApi,
  Bond as IndexerBond,
  Validator as IndexerValidator,
  VotingPower as IndexerVotingPower,
} from "@anomaorg/namada-indexer-client";
import BigNumber from "bignumber.js";
import { atomWithQuery } from "jotai-tanstack-query";
import { transparentAccountsAtom } from "./accounts";
import { chainParametersAtom } from "./chainParameters";
import { shouldUpdateBalanceAtom } from "./etc";

type Unique = {
  uuid: string;
};

export type Validator = Unique & {
  alias?: string;
  address: string;
  description?: string;
  homepageUrl?: string;
  expectedApr: number;
  unbondingPeriod: string;
  votingPowerInNAM?: BigNumber;
  votingPowerPercentage?: number;
  commission: BigNumber;
  imageUrl?: string;
};

export type MyValidator = {
  stakingStatus: string;
  stakedAmount?: BigNumber;
  unbondedAmount?: BigNumber;
  withdrawableAmount?: BigNumber;
  validator: Validator;
};

const toValidator = (
  indexerValidator: IndexerValidator,
  indexerVotingPower: IndexerVotingPower,
  unbondingPeriod: bigint
): Validator => {
  return {
    uuid: indexerValidator.address,
    alias: indexerValidator.name,
    description: indexerValidator.description,
    address: indexerValidator.address,
    homepageUrl: indexerValidator.website,
    // TODO: Return this from the indexer
    expectedApr: 0.1127,
    unbondingPeriod: `${unbondingPeriod} days`,
    votingPowerInNAM: BigNumber(indexerValidator.votingPower),
    votingPowerPercentage:
      Number(indexerValidator.votingPower) /
      Number(indexerVotingPower.totalVotingPower),
    commission: BigNumber(indexerValidator.commission),
    imageUrl: indexerValidator.avatar,
  };
};

export const allValidatorsAtom = atomWithQuery((get) => {
  const chainParameters = get(chainParametersAtom);
  return {
    queryKey: ["all-validators", chainParameters.dataUpdatedAt],
    enabled: !!chainParameters,
    queryFn: async () => {
      const parameters =
        chainParameters.data?.unbondingPeriodInDays || BigInt(0);

      const api = new DefaultApi();
      const [validatorsResponse, votingPowerResponse] = await Promise.all([
        api.apiV1PosValidatorGet(),
        api.apiV1PosVotingPowerGet(),
      ]);

      // TODO: rename one data to items?
      const validators = validatorsResponse.data.data;
      const votingPower = votingPowerResponse.data;

      return validators.map((v) => toValidator(v, votingPower, parameters));
    },
  };
});

export const myValidatorsAtom = atomWithQuery<MyValidator[]>((get) => {
  const chainParameters = get(chainParametersAtom);
  const accounts = get(transparentAccountsAtom);
  const ids = accounts.map((account) => account.address).join("-");
  // TODO: Refactor after this event subscription is enabled in the indexer
  const enablePolling = get(shouldUpdateBalanceAtom);
  return {
    queryKey: ["my-validators", ids],
    refetchInterval: enablePolling ? 1000 : false,
    queryKey: ["my-validators", ids, chainParameters.dataUpdatedAt],
    queryFn: async () => {
      const unbondingPeriod =
        chainParameters.data?.unbondingPeriodInDays || BigInt(0);
      const addresses = accounts.map((account) => account.address);
      const api = new DefaultApi();
      const bondsPromises = Promise.all(
        addresses.map((a) => api.apiV1PosBondAddressGet(a))
      );
      const [bonds, totalVotingPowerResponse] = await Promise.all([
        bondsPromises,
        api.apiV1PosVotingPowerGet(),
      ]);

      return toMyValidators(
        bonds.flatMap((b) => b.data),
        totalVotingPowerResponse.data,
        unbondingPeriod
      );
    },
  };
});
export const unbondedAmountByAddressAtom = atomWithQuery((get) =>
  deriveFromMyValidatorsAtom(
    "unbonded-amount",
    "unbondedAmount",
    get(myValidatorsAtom)
  )
);

export const withdrawableAmountByAddressAtom = atomWithQuery((get) =>
  deriveFromMyValidatorsAtom(
    "withdrawable-amount",
    "withdrawableAmount",
    get(myValidatorsAtom)
  )
);

export const stakedAmountByAddressAtom = atomWithQuery((get) =>
  deriveFromMyValidatorsAtom(
    "staked-amount",
    "stakedAmount",
    get(myValidatorsAtom)
  )
);

const deriveFromMyValidatorsAtom = (
  key: string,
  property: "stakedAmount" | "unbondedAmount" | "withdrawableAmount",
  myValidators: AtomWithQueryResult<MyValidator[], Error>
): UndefinedInitialDataOptions<Record<string, BigNumber>> => {
  return {
    queryKey: [key, myValidators.data],
    enabled: myValidators.isSuccess,
    queryFn: async () => {
      return myValidators.data!.reduce((prev, current) => {
        if (current[property]?.gt(0)) {
          return { ...prev, [current.validator.address]: current[property] };
        }
        return prev;
      }, {});
    },
  };
};

const toMyValidators = (
  indexerBonds: IndexerBond[],
  totalVotingPower: IndexerVotingPower,
  unbondingPeriod: bigint
): MyValidator[] => {
  return indexerBonds.map((indexerBond) => {
    const validator = toValidator(
      indexerBond.validator,
      totalVotingPower,
      unbondingPeriod
    );

    return {
      uuid: String(indexerBond.validator.validatorId),
      stakingStatus: "bonded",
      stakedAmount: BigNumber(indexerBond.amount),
      unbondedAmount: BigNumber(0),
      withdrawableAmount: BigNumber(0),
      validator,
    };
  });
};
