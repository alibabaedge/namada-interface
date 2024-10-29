import { SkeletonLoading, Stack } from "@namada/components";
import { AtomErrorBoundary } from "App/Common/AtomErrorBoundary";
import { BalanceChart } from "App/Common/BalanceChart";
import { FiatCurrency } from "App/Common/FiatCurrency";
import { NamCurrency } from "App/Common/NamCurrency";
import BigNumber from "bignumber.js";
import { useBalances } from "hooks/useBalances";
import { colors } from "theme";

type NamBalanceListItemProps = {
  title: string;
  color: string;
  nam?: BigNumber;
  dollar?: BigNumber | null;
  isLoading: boolean;
};

const NamBalanceListItem = ({
  title,
  color,
  nam,
  dollar,
  isLoading,
}: NamBalanceListItemProps): JSX.Element => {
  return (
    <li className="leading-5 bg-neutral-900 px-4 py-3 rounded-sm min-w-[165px]">
      <span className="flex items-center text-xs gap-1.5">
        <i className="w-2 h-2 rounded-full" style={{ background: color }} />
        {title}
      </span>
      <div className="text-lg pl-3.5">
        {isLoading && <SkeletonLoading height="22px" width="80px" />}
        {dollar === null && "N/A"}
        {dollar && <FiatCurrency amount={dollar} />}
        {nam && <NamCurrency amount={nam} currencySymbolClassName="hidden" />}
      </div>
    </li>
  );
};

export const BalanceContainer = (): JSX.Element => {
  const {
    balanceQuery,
    stakeQuery,
    isLoading,
    availableAmount,
    bondedAmount,
    shieldedAmount,
    unbondedAmount,
    withdrawableAmount,
    totalAmount,
  } = useBalances();

  return (
    <div className="flex items-center justify-center h-full w-full">
      <AtomErrorBoundary
        result={[balanceQuery, stakeQuery]}
        niceError="Unable to load balances"
      >
        <div className="flex flex-wrap md:flex-nowrap gap-4 items-center justify-center">
          <BalanceChart
            view="total"
            isLoading={isLoading}
            availableAmount={availableAmount}
            bondedAmount={bondedAmount}
            shieldedAmount={shieldedAmount || new BigNumber(0)}
            unbondedAmount={unbondedAmount}
            withdrawableAmount={withdrawableAmount}
            totalAmount={totalAmount}
          />
          <Stack gap={2} as="ul">
            <NamBalanceListItem
              title="Shielded Assets"
              color={colors.shielded}
              dollar={shieldedAmount}
              isLoading={isLoading}
            />
            <NamBalanceListItem
              title="Transparent Assets"
              color={colors.balance}
              nam={availableAmount} // TODO
              isLoading={isLoading}
            />
            <NamBalanceListItem
              title="Staked NAM"
              color={colors.bond}
              nam={bondedAmount}
              isLoading={isLoading}
            />
            <NamBalanceListItem
              title="Unbonded NAM"
              color={colors.unbond}
              nam={unbondedAmount.plus(withdrawableAmount)}
              isLoading={isLoading}
            />
          </Stack>
        </div>
      </AtomErrorBoundary>
    </div>
  );
};
