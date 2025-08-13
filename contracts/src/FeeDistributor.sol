// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract FeeDistributor {
    address public owner;
    mapping(address => uint256) public shares;
    mapping(address => uint256) public pendingBalances;
    address[] public recipients;
    uint256 public totalShares;

    uint256 public lastDistributionBlock;
    uint256 public constant DISTRIBUTION_INTERVAL = 1024; // blocks

    event SharesUpdated(address[] recipients, uint256[] shares);
    event FundsReceived(address from, uint256 amount);
    event PaymentQueued(address recipient, uint256 amount);
    event PaymentSent(address recipient, uint256 amount, bool success);
    event Distributed(uint256 blockNumber);

    modifier onlyOwner() {
        require(msg.sender == owner, "Not authorized");
        _;
    }

    constructor() {
        owner = msg.sender;
        lastDistributionBlock = block.number;
    }

    function setShares(
        address[] calldata _recipients,
        uint256[] calldata _shares
    ) external onlyOwner {
        require(_recipients.length == _shares.length, "Length mismatch");

        // Clear old data
        for (uint256 i = 0; i < recipients.length; i++) {
            delete shares[recipients[i]];
            delete pendingBalances[recipients[i]];
        }
        delete recipients;
        totalShares = 0;

        // Set new shares
        for (uint256 i = 0; i < _recipients.length; i++) {
            require(_recipients[i] != address(0), "Zero address");
            recipients.push(_recipients[i]);
            shares[_recipients[i]] = _shares[i];
            totalShares += _shares[i];
        }
        require(totalShares > 0, "No shares");

        emit SharesUpdated(_recipients, _shares);
    }

    receive() external payable {
        require(totalShares > 0, "No recipients");
        uint256 remaining = msg.value;

        // Step 1: Record pending balances
        for (uint256 i = 0; i < recipients.length; i++) {
            uint256 amount = (msg.value * shares[recipients[i]]) / totalShares;
            if (i == recipients.length - 1) {
                amount = remaining; // remainder to last
            } else {
                remaining -= amount;
            }
            pendingBalances[recipients[i]] += amount;
            emit PaymentQueued(recipients[i], amount);
        }

        emit FundsReceived(msg.sender, msg.value);

        // Step 2: Auto-distribute if time passed
        if (block.number >= lastDistributionBlock + DISTRIBUTION_INTERVAL) {
            _distributeAll();
        }
    }

    function distribute() external {
        _distributeAll();
    }

    function _distributeAll() internal {
        lastDistributionBlock = block.number;

        for (uint256 i = 0; i < recipients.length; i++) {
            uint256 amount = pendingBalances[recipients[i]];
            if (amount > 0) {
                pendingBalances[recipients[i]] = 0;
                (bool success, ) = recipients[i].call{value: amount}("");
                emit PaymentSent(recipients[i], amount, success);
            }
        }

        emit Distributed(block.number);
    }
}
