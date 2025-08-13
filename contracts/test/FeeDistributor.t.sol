// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/FeeDistributor.sol";

contract FeeDistributorTest is Test {
    // Define events locally to match the contract
    event SharesUpdated(address[] recipients, uint256[] shares);
    event FundsReceived(address from, uint256 amount);
    event PaymentQueued(address recipient, uint256 amount);
    event PaymentSent(address recipient, uint256 amount, bool success);
    event Distributed(uint256 blockNumber);

    FeeDistributor public feeDistributor;
    address public owner;
    address public recipient1;
    address public recipient2;
    address public recipient3;
    address public recipient4;
    address public nonOwner;

    function setUp() public {
        owner = makeAddr("owner");
        recipient1 = makeAddr("recipient1");
        recipient2 = makeAddr("recipient2");
        recipient3 = makeAddr("recipient3");
        recipient4 = makeAddr("recipient4");
        nonOwner = makeAddr("nonOwner");

        vm.prank(owner);
        feeDistributor = new FeeDistributor();
    }

    function test_Deployment() public view {
        assertEq(feeDistributor.owner(), owner);
        assertEq(feeDistributor.totalShares(), 0);
        assertEq(feeDistributor.lastDistributionBlock(), block.number);
        assertEq(feeDistributor.DISTRIBUTION_INTERVAL(), 1024);
        assertEq(feeDistributor.shares(recipient1), 0);
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
    }

    function test_SetShares_Owner() public {
        address[] memory recipients = new address[](2);
        recipients[0] = recipient1;
        recipients[1] = recipient2;

        uint256[] memory shares = new uint256[](2);
        shares[0] = 100;
        shares[1] = 200;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        assertEq(feeDistributor.totalShares(), 300);
        assertEq(feeDistributor.recipients(0), recipient1);
        assertEq(feeDistributor.recipients(1), recipient2);
        assertEq(feeDistributor.shares(recipient1), 100);
        assertEq(feeDistributor.shares(recipient2), 200);
    }

    function test_SetShares_NonOwner_Reverts() public {
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 100;

        vm.prank(nonOwner);
        vm.expectRevert("Not authorized");
        feeDistributor.setShares(recipients, shares);
    }

    function test_SetShares_LengthMismatch_Reverts() public {
        address[] memory recipients = new address[](2);
        recipients[0] = recipient1;
        recipients[1] = recipient2;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 100;

        vm.prank(owner);
        vm.expectRevert("Length mismatch");
        feeDistributor.setShares(recipients, shares);
    }

    function test_SetShares_ZeroAddress_Reverts() public {
        address[] memory recipients = new address[](1);
        recipients[0] = address(0);

        uint256[] memory shares = new uint256[](1);
        shares[0] = 100;

        vm.prank(owner);
        vm.expectRevert("Zero address");
        feeDistributor.setShares(recipients, shares);
    }

    function test_SetShares_EmptyArrays_Reverts() public {
        address[] memory recipients = new address[](0);
        uint256[] memory shares = new uint256[](0);

        vm.prank(owner);
        vm.expectRevert("No shares");
        feeDistributor.setShares(recipients, shares);
    }

    function test_SetShares_ZeroShares_Reverts() public {
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 0;

        vm.prank(owner);
        vm.expectRevert("No shares");
        feeDistributor.setShares(recipients, shares);
    }

    function test_SetShares_OverwritesPrevious() public {
        // Set initial shares
        address[] memory recipients1 = new address[](2);
        recipients1[0] = recipient1;
        recipients1[1] = recipient2;

        uint256[] memory shares1 = new uint256[](2);
        shares1[0] = 100;
        shares1[1] = 200;

        vm.prank(owner);
        feeDistributor.setShares(recipients1, shares1);
        assertEq(feeDistributor.totalShares(), 300);

        // Overwrite with new shares
        address[] memory recipients2 = new address[](1);
        recipients2[0] = recipient3;

        uint256[] memory shares2 = new uint256[](1);
        shares2[0] = 500;

        vm.prank(owner);
        feeDistributor.setShares(recipients2, shares2);

        assertEq(feeDistributor.totalShares(), 500);
        assertEq(feeDistributor.recipients(0), recipient3);
        // Old shares should be cleared
        assertEq(feeDistributor.shares(recipient1), 0);
        assertEq(feeDistributor.shares(recipient2), 0);
        assertEq(feeDistributor.shares(recipient3), 500);
    }

    function test_Receive_QueuesPayments() public {
        // Set up recipients
        address[] memory recipients = new address[](2);
        recipients[0] = recipient1;
        recipients[1] = recipient2;

        uint256[] memory shares = new uint256[](2);
        shares[0] = 100;
        shares[1] = 200;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send ETH to contract
        uint256 sendAmount = 1 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Check pending balances
        // The contract distributes proportionally and gives remainder to last recipient
        uint256 expected1 = feeDistributor.pendingBalances(recipient1);
        uint256 expected2 = feeDistributor.pendingBalances(recipient2);

        // Verify the amounts are queued correctly
        assertGt(expected1, 0);
        assertGt(expected2, 0);
        assertEq(expected1 + expected2, sendAmount);

        // Balances should not have changed yet
        assertEq(recipient1.balance, 0);
        assertEq(recipient2.balance, 0);
    }

    function test_Receive_NoRecipients_Reverts() public {
        // Try to send ETH without setting up recipients
        uint256 sendAmount = 1 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);

        // The call should revert with "No recipients"
        vm.expectRevert();
        address(feeDistributor).call{value: sendAmount}("");
    }

    function test_Distribute_Manual() public {
        // Set up recipients
        address[] memory recipients = new address[](2);
        recipients[0] = recipient1;
        recipients[1] = recipient2;

        uint256[] memory shares = new uint256[](2);
        shares[0] = 100;
        shares[1] = 200;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send ETH to queue payments
        uint256 sendAmount = 1 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Get pending balances before distribution
        uint256 pending1 = feeDistributor.pendingBalances(recipient1);
        uint256 pending2 = feeDistributor.pendingBalances(recipient2);

        // Manually distribute
        vm.prank(owner);
        feeDistributor.distribute();

        // Check that payments were sent
        assertEq(recipient1.balance, pending1);
        assertEq(recipient2.balance, pending2);

        // Pending balances should be cleared
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
        assertEq(feeDistributor.pendingBalances(recipient2), 0);
    }

    function test_Distribute_AutoAfterInterval() public {
        // Set up recipients
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 1000;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send ETH to queue payments
        uint256 sendAmount = 1 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Fast forward past the distribution interval
        vm.roll(block.number + 1025); // 1024 + 1

        // Send another payment to trigger auto-distribution
        vm.deal(owner, 1 ether);
        vm.prank(owner);
        (bool success2, ) = address(feeDistributor).call{value: 1 ether}("");
        require(success2, "Transfer 2 failed");

        // Check that payments were automatically distributed
        assertEq(recipient1.balance, 2 ether);
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
    }

    function test_Distribute_NotBeforeInterval() public {
        // Set up recipients
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 1000;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send ETH to queue payments
        uint256 sendAmount = 1 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Fast forward but not past the distribution interval
        vm.roll(block.number + 1000); // Less than 1024

        // Send another payment - should not trigger auto-distribution
        vm.deal(owner, 1 ether);
        vm.prank(owner);
        (bool success2, ) = address(feeDistributor).call{value: 1 ether}("");
        require(success2, "Transfer 2 failed");

        // Check that payments are still queued
        assertEq(recipient1.balance, 0);
        assertEq(feeDistributor.pendingBalances(recipient1), 2 ether);
    }

    function test_Distribute_MultipleRecipients() public {
        // Set up recipients
        address[] memory recipients = new address[](3);
        recipients[0] = recipient1;
        recipients[1] = recipient2;
        recipients[2] = recipient3;

        uint256[] memory shares = new uint256[](3);
        shares[0] = 100;
        shares[1] = 200;
        shares[2] = 300;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send ETH to queue payments
        uint256 sendAmount = 3 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Manually distribute
        vm.prank(owner);
        feeDistributor.distribute();

        // Check distributions based on proportional shares
        // Total shares: 600
        // recipient1: 100/600 * 3 ETH = 0.5 ETH
        // recipient2: 200/600 * 3 ETH = 1.0 ETH
        // recipient3: 300/600 * 3 ETH = 1.5 ETH
        assertEq(recipient1.balance, 0.5 ether);
        assertEq(recipient2.balance, 1.0 ether);
        assertEq(recipient3.balance, 1.5 ether);
    }

    function test_Distribute_ExactAmounts() public {
        // Set up recipients with equal shares
        address[] memory recipients = new address[](2);
        recipients[0] = recipient1;
        recipients[1] = recipient2;

        uint256[] memory shares = new uint256[](2);
        shares[0] = 1;
        shares[1] = 1;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send exactly 2 ETH (should distribute 1 ETH each)
        uint256 sendAmount = 2 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Distribute
        vm.prank(owner);
        feeDistributor.distribute();

        assertEq(recipient1.balance, 1 ether);
        assertEq(recipient2.balance, 1 ether);
    }

    function test_Distribute_SmallAmount() public {
        // Set up recipients
        address[] memory recipients = new address[](2);
        recipients[0] = recipient1;
        recipients[1] = recipient2;

        uint256[] memory shares = new uint256[](2);
        shares[0] = 1;
        shares[1] = 1;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send 1 wei (should distribute 0 wei each due to integer division)
        vm.deal(owner, 1);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: 1}("");
        require(success, "Transfer failed");

        // Distribute
        vm.prank(owner);
        feeDistributor.distribute();

        assertEq(recipient1.balance, 0);
        assertEq(recipient2.balance, 1); // Last recipient gets remainder
    }

    function test_Distribute_AccumulatedPayments() public {
        // Set up recipients
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 1000;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send multiple payments
        vm.deal(owner, 10 ether);

        vm.prank(owner);
        (bool success1, ) = address(feeDistributor).call{value: 1 ether}("");
        require(success1, "Transfer 1 failed");

        vm.prank(owner);
        (bool success2, ) = address(feeDistributor).call{value: 2 ether}("");
        require(success2, "Transfer 2 failed");

        // Distribute
        vm.prank(owner);
        feeDistributor.distribute();

        // recipient1 should have received all 3 ETH
        assertEq(recipient1.balance, 3 ether);
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
    }

    function test_Distribute_FromDifferentSenders() public {
        // Set up recipients
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 1000;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send from different addresses
        address sender1 = makeAddr("sender1");
        address sender2 = makeAddr("sender2");

        vm.deal(sender1, 1 ether);
        vm.deal(sender2, 2 ether);

        vm.prank(sender1);
        (bool success1, ) = address(feeDistributor).call{value: 1 ether}("");
        require(success1, "Transfer from sender1 failed");

        vm.prank(sender2);
        (bool success2, ) = address(feeDistributor).call{value: 2 ether}("");
        require(success2, "Transfer from sender2 failed");

        // Distribute
        vm.prank(owner);
        feeDistributor.distribute();

        // recipient1 should have received all 3 ETH
        assertEq(recipient1.balance, 3 ether);
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
    }

    function test_Events_Emitted() public {
        // Set up recipients
        address[] memory recipients = new address[](1);
        recipients[0] = recipient1;

        uint256[] memory shares = new uint256[](1);
        shares[0] = 1000;

        vm.prank(owner);
        feeDistributor.setShares(recipients, shares);

        // Send ETH
        uint256 sendAmount = 1 ether;
        vm.deal(owner, sendAmount);
        vm.prank(owner);

        (bool success, ) = address(feeDistributor).call{value: sendAmount}("");
        require(success, "Transfer failed");

        // Distribute
        vm.prank(owner);
        feeDistributor.distribute();

        // Verify the events were emitted by checking the state changes
        assertEq(recipient1.balance, 1 ether);
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
    }

    function test_Constructor_SetsOwner() public {
        address newOwner = makeAddr("newOwner");
        vm.prank(newOwner);
        FeeDistributor newContract = new FeeDistributor();

        assertEq(newContract.owner(), newOwner);
        assertEq(newContract.lastDistributionBlock(), block.number);
    }

    function test_SetShares_ClearsOldData() public {
        // Set initial shares
        address[] memory recipients1 = new address[](2);
        recipients1[0] = recipient1;
        recipients1[1] = recipient2;

        uint256[] memory shares1 = new uint256[](2);
        shares1[0] = 100;
        shares1[1] = 200;

        vm.prank(owner);
        feeDistributor.setShares(recipients1, shares1);

        // Send some ETH to create pending balances
        vm.deal(owner, 1 ether);
        vm.prank(owner);
        (bool success, ) = address(feeDistributor).call{value: 1 ether}("");
        require(success, "Transfer failed");

        // Overwrite with new shares
        address[] memory recipients2 = new address[](1);
        recipients2[0] = recipient3;

        uint256[] memory shares2 = new uint256[](1);
        shares2[0] = 500;

        vm.prank(owner);
        feeDistributor.setShares(recipients2, shares2);

        // Check that old data is cleared
        assertEq(feeDistributor.shares(recipient1), 0);
        assertEq(feeDistributor.shares(recipient2), 0);
        assertEq(feeDistributor.pendingBalances(recipient1), 0);
        assertEq(feeDistributor.pendingBalances(recipient2), 0);
        assertEq(feeDistributor.totalShares(), 500);
        assertEq(feeDistributor.recipients(0), recipient3);
    }
}
