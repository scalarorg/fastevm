// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/FeeDistributor.sol";

contract DeployScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);

        FeeDistributor feeDistributor = new FeeDistributor();

        vm.stopBroadcast();

        console.log("FeeDistributor deployed to:", address(feeDistributor));
        console.log(
            "Contract ABI available in out/FeeDistributor.sol/FeeDistributor.json"
        );
        console.log("");
        console.log(
            "To configure recipients, call setShares from owner address:"
        );
        console.log(
            "await feeDistributor.setShares([address1, address2], [100, 200]);"
        );
    }
}
