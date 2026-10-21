use crate::genesis::types::AllocationPurpose;
use crate::genesis::vesting::VestingSchedule;
use crate::types::{Address, Amount, BlockHeight};

pub fn make_vest(
    total_amount: Amount,
    start_height: BlockHeight,
    cliff_height: BlockHeight,
    end_height: BlockHeight,
) -> VestingSchedule {
    VestingSchedule {
        beneficiary: Address::from([0xAA; 20]),
        total_amount,
        released: 0,
        start_height,
        cliff_height,
        end_height,
        purpose: AllocationPurpose::Team,
    }
}
